//! Installation of optional third-party biology and chemistry tools.
//!
//! The installer owns the orchestration that used to live in application-specific shell and
//! PowerShell scripts: isolated Python environments, CPU/CUDA PyTorch selection, downloads,
//! source checkouts, model assets, and post-install verification. Callers choose the outer data
//! directory and can therefore share the same recipes without sharing application-specific path
//! discovery or UI code.
//!
//! Commands are always launched directly with [`std::process::Command`]. A shell is used only when
//! an upstream project itself distributes a shell installer.

use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use crate::{run::CommandSpec, status};

mod alphafold3;
mod boltz2;
mod boltzgen;
mod common;
mod conda_tools;
mod igblast;
mod opendde;
mod protein_mpnn;
mod python_tools;
mod uninstall;

pub use uninstall::UninstallReport;

use crate::tool_definitions::Tool;

/// How per-tool environments and non-Python assets are laid out.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallLayout {
    /// Root containing source checkouts, binary distributions, and model data.
    pub tools_root: PathBuf,
    /// Root containing each isolated Python/micromamba environment.
    pub environments_root: PathBuf,
    /// Appended to a tool slug when naming its environment.
    pub environment_suffix: String,
}

impl InstallLayout {
    /// Layout used by Molchanica: `<root>/<slug>-venv` and `<root>/tools/<bundle>`.
    pub fn managed(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self {
            tools_root: root.join("tools"),
            environments_root: root,
            environment_suffix: "-venv".to_owned(),
        }
    }

    /// Layout used by applications that keep environments under a separate directory.
    pub fn split(tools_root: impl Into<PathBuf>, environments_root: impl Into<PathBuf>) -> Self {
        Self {
            tools_root: tools_root.into(),
            environments_root: environments_root.into(),
            environment_suffix: String::new(),
        }
    }

    /// Shared application layout: assets live directly under process_executables, while
    /// isolated Python and micromamba environments live under process_executables/python_envs.
    pub fn process_executables(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        Self::split(root.clone(), root.join("python_envs"))
    }

    pub fn environment(&self, slug: &str) -> PathBuf {
        self.environments_root
            .join(format!("{slug}{}", self.environment_suffix))
    }
}

/// Requested PyTorch wheel family.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TorchBackendPreference {
    #[default]
    Auto,
    Cpu,
    Cuda126,
}

impl FromStr for TorchBackendPreference {
    type Err = InstallError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "cpu" => Ok(Self::Cpu),
            "cuda" | "cu126" | "cuda126" => Ok(Self::Cuda126),
            _ => Err(InstallError::InvalidConfiguration(
                "the torch backend must be auto, cpu, or cu126".to_owned(),
            )),
        }
    }
}

/// Configuration shared by all installation recipes.
#[derive(Clone, Debug)]
pub struct InstallConfig {
    pub layout: InstallLayout,
    pub torch_backend: TorchBackendPreference,
    pub uv_executable: Option<PathBuf>,
    /// Only consulted by the recipes that hand control to an upstream `install.sh`; everything
    /// else resolves [`InstallConfig::micromamba_executable`] instead.
    pub conda_executable: Option<PathBuf>,
    pub micromamba_executable: Option<PathBuf>,
    /// Project/release root containing optional adapter helper scripts.
    pub support_root: Option<PathBuf>,
    pub opendde_root: Option<PathBuf>,
    pub prewarm_opendde: bool,
    pub igblast_version: String,
    pub netsolp_models_url: Option<String>,
    pub gromacs_version: String,
    pub gromacs_prefix: Option<PathBuf>,
}

impl InstallConfig {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            layout: InstallLayout::managed(root),
            torch_backend: TorchBackendPreference::Auto,
            uv_executable: None,
            conda_executable: None,
            micromamba_executable: None,
            support_root: None,
            opendde_root: None,
            prewarm_opendde: true,
            igblast_version: "1.22.0".to_owned(),
            netsolp_models_url: None,
            gromacs_version: "2026.3".to_owned(),
            gromacs_prefix: None,
        }
    }

    /// Apply the compatibility environment variables used by the two original installers.
    pub fn apply_environment(mut self) -> Result<Self, InstallError> {
        if self.uv_executable.is_none() {
            self.uv_executable = first_env_path(&["BIO_TOOLS_UV", "MOLCHANICA_UV"]);
        }
        if self.conda_executable.is_none() {
            self.conda_executable = first_env_path(&["BIO_TOOLS_CONDA"]);
        }
        if self.micromamba_executable.is_none() {
            self.micromamba_executable = first_env_path(&["BIO_TOOLS_MICROMAMBA"]);
        }
        if let Some(value) = first_env(&[
            "BIO_TOOLS_TORCH_BACKEND",
            "MOLCHANICA_TORCH_BACKEND",
            "BIO_WEB_TORCH_BACKEND",
        ]) {
            self.torch_backend = value.parse()?;
        }
        if self.opendde_root.is_none() {
            self.opendde_root = first_env_path(&["OPENDDE_ROOT_DIR"]);
        }
        if let Some(value) = first_env(&["IGBLAST_VERSION"]) {
            self.igblast_version = value;
        }
        if self.netsolp_models_url.is_none() {
            self.netsolp_models_url = first_env(&["NETSOLP_MODELS_URL"]);
        }
        if let Some(value) = first_env(&["GROMACS_VERSION"]) {
            self.gromacs_version = value;
        }
        if self.gromacs_prefix.is_none() {
            self.gromacs_prefix = first_env_path(&["GROMACS_INSTALL_PREFIX"]);
        }
        Ok(self)
    }
}

fn first_env(names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
}

fn first_env_path(names: &[&str]) -> Option<PathBuf> {
    first_env(names).map(PathBuf::from)
}

/// Machine-readable outcome of a tool availability probe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusKind {
    Pass,
    NotFound,
    Error,
}

/// Result returned by [Installer::status].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolStatus {
    pub result: StatusKind,
    pub detail: String,
    /// GPU or CPU when the installed runtime can report it.
    pub device: Option<String>,
}

/// Progress emitted at stable tool/step boundaries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstallEvent {
    ToolStarted(Tool),
    Step { tool: Tool, description: String },
    Note { tool: Option<Tool>, message: String },
    ToolFinished(Tool),
}

/// Error from a recipe or one of its direct child processes.
#[derive(Debug)]
pub enum InstallError {
    Unsupported {
        tool: Tool,
        reason: String,
    },
    InvalidConfiguration(String),
    Io {
        action: String,
        source: std::io::Error,
    },
    Command {
        command: String,
        status: Option<i32>,
    },
    Download {
        url: String,
        message: String,
    },
}

impl InstallError {
    pub(crate) fn io(action: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            action: action.into(),
            source,
        }
    }
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { tool, reason } => write!(f, "cannot install {tool}: {reason}"),
            Self::InvalidConfiguration(message) => f.write_str(message),
            Self::Io { action, source } => write!(f, "{action}: {source}"),
            Self::Command { command, status } => match status {
                Some(code) => write!(f, "`{command}` exited with status {code}"),
                None => write!(
                    f,
                    "`{command}` was terminated before reporting an exit status"
                ),
            },
            Self::Download { url, message } => write!(f, "unable to download {url}: {message}"),
        }
    }
}

impl Error for InstallError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// One failure in a multi-tool installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallFailure {
    pub tool: Tool,
    pub error: String,
}

/// Results from [`Installer::install_many`]. Each recipe is attempted independently.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InstallReport {
    pub installed: Vec<Tool>,
    pub failed: Vec<InstallFailure>,
}

impl InstallReport {
    pub fn is_success(&self) -> bool {
        self.failed.is_empty()
    }
}

type Reporter = Arc<dyn Fn(InstallEvent) + Send + Sync>;

/// Stateful installation context. The located uv/micromamba/Conda executables and selected Torch
/// backend are cached across recipes in one run.
pub struct Installer {
    pub config: InstallConfig,
    reporter: Option<Reporter>,
    current_tool: Option<Tool>,
    uv: Option<PathBuf>,
    micromamba: Option<PathBuf>,
    conda: Option<PathBuf>,
    /// Anaconda's terms only need accepting once per run, and only on the Conda path.
    conda_terms_accepted: bool,
    torch_backend: Option<common::TorchBackend>,
}

impl Installer {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::from_config(InstallConfig::new(root))
    }

    /// Construct an installer using the compatibility environment variables documented by the
    /// original Molchanica and bio_web installers.
    pub fn from_environment(root: impl Into<PathBuf>) -> Result<Self, InstallError> {
        Ok(Self::from_config(
            InstallConfig::new(root).apply_environment()?,
        ))
    }
    /// Construct an installer for the shared process_executables/python_envs layout.
    pub fn for_process_executables(root: impl Into<PathBuf>) -> Result<Self, InstallError> {
        let root = root.into();
        let mut config = InstallConfig::new(&root).apply_environment()?;
        config.layout = InstallLayout::process_executables(root);
        Ok(Self::from_config(config))
    }

    pub fn from_config(config: InstallConfig) -> Self {
        Self {
            config,
            reporter: None,
            current_tool: None,
            uv: None,
            micromamba: None,
            conda: None,
            conda_terms_accepted: false,
            torch_backend: None,
        }
    }

    pub fn with_reporter(
        mut self,
        reporter: impl Fn(InstallEvent) + Send + Sync + 'static,
    ) -> Self {
        self.reporter = Some(Arc::new(reporter));
        self
    }

    pub fn environment_path(&self, tool: Tool) -> PathBuf {
        self.config.layout.environment(tool.slug())
    }

    /// Locate a tool's installed console entry point in its managed environment.
    pub fn executable_path(&self, tool: Tool) -> PathBuf {
        self.venv_script(tool.slug(), tool.console_script())
    }
    /// Build a command for a tool's console entry point in its managed environment.
    ///
    /// The child receives `VIRTUAL_ENV` and a `PATH` whose first directory is the
    /// tool environment's scripts directory. This is equivalent to activating the
    /// environment before running its console script, and lets dependencies such
    /// as PyTorch find helper executables installed beside the script.
    pub fn tool_command(&self, tool: Tool) -> CommandSpec {
        self.with_tool_environment(tool, CommandSpec::new(self.executable_path(tool)))
    }

    /// Build a command for the managed Python interpreter of a tool.
    pub fn tool_python_command(&self, tool: Tool) -> CommandSpec {
        self.with_tool_environment(tool, CommandSpec::new(self.venv_python(tool.slug())))
    }

    fn with_tool_environment(&self, tool: Tool, command: CommandSpec) -> CommandSpec {
        let environment = self.venv_dir(tool.slug());
        let scripts = self.venv_scripts_dir(tool.slug());
        let inherited = std::env::var_os("PATH")
            .map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
            .unwrap_or_default();
        let path = std::env::join_paths(std::iter::once(scripts.clone()).chain(inherited))
            .unwrap_or_else(|_| scripts.into_os_string());
        command.env("VIRTUAL_ENV", environment).env("PATH", path)
    }

    pub fn tools_root(&self) -> &Path {
        &self.config.layout.tools_root
    }

    /// Install or refresh one tool. Recipes are designed to be safely rerunnable.
    pub fn install(&mut self, tool: Tool) -> Result<(), InstallError> {
        if !tool.is_supported() {
            return Err(InstallError::Unsupported {
                tool,
                reason: "the required upstream wheels or binaries are Linux-only".to_owned(),
            });
        }

        self.current_tool = Some(tool);
        self.emit(InstallEvent::ToolStarted(tool));
        let result = match tool {
            Tool::AlphaFold3 => alphafold3::install(self),
            Tool::OpenDde => opendde::install(self),
            Tool::Boltz2 => boltz2::install(self),
            Tool::BoltzGen => boltzgen::install(self),
            Tool::ProteinMpnn => protein_mpnn::install_protein(self),
            Tool::LigandMpnn => protein_mpnn::install_ligand(self),
            Tool::IgBlast => igblast::install(self),
            Tool::HighFold
            | Tool::BindCraft
            | Tool::AntiFold
            | Tool::Germinal
            | Tool::Mber
            | Tool::Genie3
            | Tool::AggreScan3d => conda_tools::install(self, tool),
            _ => python_tools::install(self, tool),
        };
        if result.is_ok() {
            if let Err(error) = status::record_install(self, tool) {
                self.note(format!("Unable to record installation status: {error}"));
            }
            self.emit(InstallEvent::ToolFinished(tool));
        }
        self.current_tool = None;
        result
    }

    /// Probe an installed tool using the same layout used to install it.
    pub fn status(&self, tool: Tool) -> ToolStatus {
        status::check(self, tool)
    }

    /// Probe every tool installed by this crate using this installer's layout.
    pub fn list(&self) -> Vec<(Tool, ToolStatus)> {
        status::list(self)
    }
    /// Remove one tool's environment, assets, and installation marker.
    ///
    /// Rerunnable in the same sense the recipes are: a tool that is already absent uninstalls
    /// successfully with an empty report, which is what makes this safe to offer as a button
    /// beside a status that may be a few minutes stale.
    pub fn uninstall(&mut self, tool: Tool) -> Result<UninstallReport, InstallError> {
        self.current_tool = Some(tool);
        self.emit(InstallEvent::ToolStarted(tool));
        let result = uninstall::uninstall(self, tool);
        if result.is_ok() {
            self.emit(InstallEvent::ToolFinished(tool));
        }
        self.current_tool = None;
        result
    }

    /// Install several tools without letting one broken upstream release suppress the rest.
    pub fn install_many(&mut self, tools: impl IntoIterator<Item = Tool>) -> InstallReport {
        let mut report = InstallReport::default();
        for tool in tools {
            match self.install(tool) {
                Ok(()) => report.installed.push(tool),
                Err(error) => report.failed.push(InstallFailure {
                    tool,
                    error: error.to_string(),
                }),
            }
        }
        report
    }

    pub(crate) fn step(&self, description: impl Into<String>) {
        if let Some(tool) = self.current_tool {
            self.emit(InstallEvent::Step {
                tool,
                description: description.into(),
            });
        }
    }

    pub(crate) fn note(&self, message: impl Into<String>) {
        self.emit(InstallEvent::Note {
            tool: self.current_tool,
            message: message.into(),
        });
    }

    fn emit(&self, event: InstallEvent) {
        if let Some(reporter) = &self.reporter {
            reporter(event);
            return;
        }
        match event {
            InstallEvent::ToolStarted(tool) => println!("\n{tool}\n{}", "=".repeat(56)),
            InstallEvent::Step { description, .. } => println!("  {description}"),
            InstallEvent::Note { message, .. } => println!("  {message}"),
            InstallEvent::ToolFinished(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_command_activates_its_managed_environment() {
        let installer = Installer::new("managed-root");
        let command = installer.tool_command(Tool::OpenDde);
        assert_eq!(
            command.environment.get(std::ffi::OsStr::new("VIRTUAL_ENV")),
            Some(&installer.venv_dir(Tool::OpenDde.slug()).into_os_string())
        );
        let path = command
            .environment
            .get(std::ffi::OsStr::new("PATH"))
            .unwrap();
        assert_eq!(
            std::env::split_paths(path).next(),
            Some(installer.venv_scripts_dir(Tool::OpenDde.slug()))
        );
    }
    #[test]
    fn all_tools_contains_no_duplicates() {
        let unique: std::collections::HashSet<_> = Tool::ALL.into_iter().collect();
        assert_eq!(unique.len(), Tool::ALL.len());
    }
    #[test]
    fn every_tool_round_trips_through_its_slug() {
        for tool in Tool::ALL {
            assert_eq!(tool.slug().parse::<Tool>().unwrap(), tool);
            assert_eq!(tool.name().parse::<Tool>().unwrap(), tool);
        }
    }

    #[test]
    fn split_and_managed_layouts_are_explicit() {
        let managed = InstallLayout::managed("/data/app");
        assert_eq!(
            managed.environment("boltz2"),
            Path::new("/data/app/boltz2-venv")
        );
        assert_eq!(managed.tools_root, Path::new("/data/app/tools"));

        let split = InstallLayout::split("/data/tools", "/data/envs");
        assert_eq!(split.environment("boltz2"), Path::new("/data/envs/boltz2"));
    }

    #[test]
    fn consumer_aliases_parse_to_the_canonical_tool() {
        assert_eq!("boltz".parse::<Tool>().unwrap(), Tool::Boltz2);
        assert_eq!("esmfold".parse::<Tool>().unwrap(), Tool::EsmFold2);
        assert_eq!("antibody_annotator".parse::<Tool>().unwrap(), Tool::Anarcii);
        assert_eq!("AbMPNN".parse::<Tool>().unwrap(), Tool::ProteinMpnn);
    }
}
