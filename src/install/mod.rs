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

use crate::status;

mod alphafold3;
mod boltz2;
mod boltzgen;
mod common;
mod conda_tools;
mod igblast;
mod opendde;
mod protein_mpnn;
mod python_tools;

/// A tool with an unattended or partially unattended installation recipe.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Tool {
    AlphaFold3,
    OpenDde,
    Boltz2,
    Chai1,
    Protenix,
    EsmFold2,
    ImmuneBuilder,
    HighFold,
    BoltzGen,
    BindCraft,
    IgBlast,
    BioPhi,
    AntiFold,
    ProteinMpnn,
    LigandMpnn,
    ProteinMpnnDdg,
    RfDiffusion,
    RfAntibody,
    Germinal,
    Mber,
    IgDesign,
    ThermoMpnn,
    Genie3,
    DeepSp,
    DeepImmuno,
    TlImmuno2,
    NetSolP,
    DeepStabP,
    AggreScan3d,
    DlkCat,
    CatPred,
    Anarcii,
    Placer,
    Gromacs,
    BoltzAdme,
}

impl Tool {
    /// Every recipe in a stable, user-facing order.
    pub const ALL: [Self; 35] = [
        Self::AlphaFold3,
        Self::OpenDde,
        Self::Boltz2,
        Self::Chai1,
        Self::Protenix,
        Self::EsmFold2,
        Self::ImmuneBuilder,
        Self::HighFold,
        Self::BoltzGen,
        Self::BindCraft,
        Self::LigandMpnn,
        Self::ProteinMpnn,
        Self::ProteinMpnnDdg,
        Self::RfDiffusion,
        Self::RfAntibody,
        Self::Germinal,
        Self::Mber,
        Self::IgDesign,
        Self::ThermoMpnn,
        Self::Genie3,
        Self::DeepSp,
        Self::DeepImmuno,
        Self::TlImmuno2,
        Self::NetSolP,
        Self::DeepStabP,
        Self::AggreScan3d,
        Self::DlkCat,
        Self::CatPred,
        Self::IgBlast,
        Self::BioPhi,
        Self::Anarcii,
        Self::Placer,
        Self::Gromacs,
        Self::BoltzAdme,
        Self::AntiFold,
    ];

    /// Stable machine-readable name used for environment and checkout paths.
    pub const fn slug(self) -> &'static str {
        match self {
            Self::AlphaFold3 => "alphafold3",
            Self::OpenDde => "opendde",
            Self::Boltz2 => "boltz2",
            Self::Chai1 => "chai1",
            Self::Protenix => "protenix",
            Self::EsmFold2 => "esmfold2",
            Self::ImmuneBuilder => "immunebuilder",
            Self::HighFold => "highfold",
            Self::BoltzGen => "boltzgen",
            Self::BindCraft => "bindcraft",
            Self::IgBlast => "igblast",
            Self::BioPhi => "biophi",
            Self::AntiFold => "antifold",
            Self::ProteinMpnn => "proteinmpnn",
            Self::LigandMpnn => "ligandmpnn",
            Self::ProteinMpnnDdg => "proteinmpnn-ddg",
            Self::RfDiffusion => "rfdiffusion",
            Self::RfAntibody => "rfantibody",
            Self::Germinal => "germinal",
            Self::Mber => "mber",
            Self::IgDesign => "igdesign",
            Self::ThermoMpnn => "thermompnn",
            Self::Genie3 => "genie3",
            Self::DeepSp => "deepsp",
            Self::DeepImmuno => "deepimmuno",
            Self::TlImmuno2 => "tlimmuno2",
            Self::NetSolP => "netsolp",
            Self::DeepStabP => "deepstabp",
            Self::AggreScan3d => "aggrescan3d",
            Self::DlkCat => "dlkcat",
            Self::CatPred => "catpred",
            Self::Anarcii => "anarcii",
            Self::Placer => "placer",
            Self::Gromacs => "gromacs",
            Self::BoltzAdme => "boltz-adme",
        }
    }

    /// Human-readable upstream name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::AlphaFold3 => "AlphaFold 3",
            Self::OpenDde => "OpenDDE",
            Self::Boltz2 => "Boltz-2",
            Self::Chai1 => "Chai-1",
            Self::Protenix => "Protenix-v2",
            Self::EsmFold2 => "ESMFold 2",
            Self::ImmuneBuilder => "ImmuneBuilder",
            Self::HighFold => "HighFold",
            Self::BoltzGen => "BoltzGen",
            Self::BindCraft => "BindCraft",
            Self::IgBlast => "IgBLAST",
            Self::BioPhi => "BioPhi",
            Self::AntiFold => "AntiFold",
            Self::ProteinMpnn => "ProteinMPNN",
            Self::LigandMpnn => "LigandMPNN",
            Self::ProteinMpnnDdg => "ProteinMPNN-ddG",
            Self::RfDiffusion => "RFdiffusion",
            Self::RfAntibody => "RFantibody",
            Self::Germinal => "Germinal",
            Self::Mber => "mBER",
            Self::IgDesign => "IgDesign",
            Self::ThermoMpnn => "ThermoMPNN",
            Self::Genie3 => "Genie 3",
            Self::DeepSp => "DeepSP",
            Self::DeepImmuno => "DeepImmuno",
            Self::TlImmuno2 => "TLimmuno2",
            Self::NetSolP => "NetSolP",
            Self::DeepStabP => "DeepSTABp",
            Self::AggreScan3d => "AggreScan3D",
            Self::DlkCat => "DLKcat",
            Self::CatPred => "CatPred",
            Self::Anarcii => "ANARCII",
            Self::Placer => "PLACER",
            Self::Gromacs => "GROMACS",
            Self::BoltzAdme => "Boltz ADME",
        }
    }

    /// Whether upstream publishes the required binaries and wheels for the current platform.
    pub const fn is_supported(self) -> bool {
        cfg!(target_os = "linux")
            || !matches!(
                self,
                Self::AlphaFold3
                    | Self::Chai1
                    | Self::Protenix
                    | Self::EsmFold2
                    | Self::HighFold
                    | Self::BoltzGen
                    | Self::BindCraft
                    | Self::AntiFold
                    | Self::ProteinMpnnDdg
                    | Self::RfDiffusion
                    | Self::RfAntibody
                    | Self::Germinal
                    | Self::Mber
                    | Self::IgDesign
                    | Self::Genie3
                    | Self::AggreScan3d
                    | Self::CatPred
                    | Self::Placer
                    | Self::Gromacs
            )
    }

    /// Recipes supported on the current operating system.
    pub fn supported() -> impl Iterator<Item = Self> {
        Self::ALL.into_iter().filter(|tool| tool.is_supported())
    }
}

impl fmt::Display for Tool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Tool {
    type Err = InstallError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        // Consumer registries use a mixture of display names, executable names, hyphens, and
        // underscores. Treat punctuation as presentation so `Process::install` can resolve either
        // form without every application maintaining another alias table.
        let normalized: String = value
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect();
        let tool = match normalized.as_str() {
            "alphafold3" => Self::AlphaFold3,
            "opendde" => Self::OpenDde,
            "boltz" | "boltz2" => Self::Boltz2,
            "chai1" => Self::Chai1,
            "protenix" | "protenixv2" => Self::Protenix,
            "esmfold" | "esmfold2" => Self::EsmFold2,
            "immunebuilder" => Self::ImmuneBuilder,
            "highfold" => Self::HighFold,
            "boltzgen" => Self::BoltzGen,
            "bindcraft" => Self::BindCraft,
            "igblast" => Self::IgBlast,
            "biophi" => Self::BioPhi,
            "antifold" => Self::AntiFold,
            "proteinmpnn" | "abmpnn" => Self::ProteinMpnn,
            "ligandmpnn" => Self::LigandMpnn,
            "proteinmpnnddg" => Self::ProteinMpnnDdg,
            "rfdiffusion" => Self::RfDiffusion,
            "rfantibody" => Self::RfAntibody,
            "germinal" => Self::Germinal,
            "mber" => Self::Mber,
            "igdesign" => Self::IgDesign,
            "thermompnn" => Self::ThermoMpnn,
            "genie3" => Self::Genie3,
            "deepsp" => Self::DeepSp,
            "deepimmuno" => Self::DeepImmuno,
            "tlimmuno" | "tlimmuno2" => Self::TlImmuno2,
            "netsolp" => Self::NetSolP,
            "deepstabp" => Self::DeepStabP,
            "aggrescan3d" => Self::AggreScan3d,
            "dlkcat" => Self::DlkCat,
            "catpred" => Self::CatPred,
            "anarcii" | "antibodyannotator" => Self::Anarcii,
            "placer" => Self::Placer,
            "gromacs" => Self::Gromacs,
            "boltzadme" => Self::BoltzAdme,
            _ => {
                return Err(InstallError::InvalidConfiguration(format!(
                    "unknown tool slug {value:?}"
                )));
            }
        };
        Ok(tool)
    }
}

/// How per-tool environments and non-Python assets are laid out.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallLayout {
    /// Root containing source checkouts, binary distributions, and model data.
    pub tools_root: PathBuf,
    /// Root containing each isolated Python/Conda environment.
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
    /// isolated Python and Conda environments live under process_executables/python_envs.
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
    pub conda_executable: Option<PathBuf>,
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

/// Stateful installation context. The located uv/Conda executables and selected Torch backend are
/// cached across recipes in one run.
pub struct Installer {
    pub config: InstallConfig,
    reporter: Option<Reporter>,
    current_tool: Option<Tool>,
    uv: Option<PathBuf>,
    conda: Option<PathBuf>,
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
            conda: None,
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
