use std::{
    env,
    ffi::{OsStr, OsString},
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use super::{InstallError, Installer, TorchBackendPreference};
use crate::tool_definitions::Tool;

const CUDA_126_INDEX: &str = "https://download.pytorch.org/whl/cu126";
const CPU_TORCH_INDEX: &str = "https://download.pytorch.org/whl/cpu";
const MICROMAMBA_RELEASES: &str =
    "https://github.com/mamba-org/micromamba-releases/releases/latest/download";
/// micromamba ships no default channels, so every explicit spec has to name one. Staying on
/// conda-forge also keeps us clear of the Anaconda `defaults` licence terms.
pub(crate) const CONDA_FORGE: &str = "conda-forge";
const PYTHON_ENVIRONMENT_VARIABLES: [&str; 7] = [
    "VIRTUAL_ENV",
    "UV_PROJECT_ENVIRONMENT",
    "CONDA_PREFIX",
    "PYTHONHOME",
    "PYTHONPATH",
    "UV_PYTHON",
    "UV_NO_MANAGED_PYTHON",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TorchBackend {
    Cpu,
    Cuda126,
}

impl TorchBackend {
    pub(crate) const fn index_url(self) -> &'static str {
        match self {
            Self::Cpu => CPU_TORCH_INDEX,
            Self::Cuda126 => CUDA_126_INDEX,
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Cuda126 => "CUDA 12.6",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PipOptions<'a> {
    pub(crate) index_url: Option<&'a str>,
    pub(crate) extra_indexes: &'a [&'a str],
    pub(crate) index_strategy: Option<&'a str>,
    pub(crate) no_build_isolation: bool,
    pub(crate) upgrade: bool,
    pub(crate) extra_env: &'a [(&'a str, &'a str)],
}

impl Default for PipOptions<'_> {
    fn default() -> Self {
        Self {
            index_url: None,
            extra_indexes: &[],
            index_strategy: None,
            no_build_isolation: false,
            upgrade: true,
            extra_env: &[],
        }
    }
}

impl Installer {
    pub(crate) fn venv_dir(&self, slug: &str) -> PathBuf {
        if slug == "opendde"
            && let Some(path) = env::var_os("OPENDDE_VENV_DIR")
        {
            return PathBuf::from(path);
        }
        self.config.layout.environment(slug)
    }

    pub(crate) fn venv_python(&self, slug: &str) -> PathBuf {
        let directory = self.venv_dir(slug);
        if cfg!(target_os = "windows") {
            directory.join("Scripts").join("python.exe")
        } else {
            directory.join("bin").join("python")
        }
    }

    pub(crate) fn venv_scripts_dir(&self, slug: &str) -> PathBuf {
        self.venv_dir(slug).join(if cfg!(target_os = "windows") {
            "Scripts"
        } else {
            "bin"
        })
    }

    pub(crate) fn venv_script(&self, slug: &str, script: &str) -> PathBuf {
        let directory = self.venv_scripts_dir(slug);
        if !cfg!(target_os = "windows") {
            return directory.join(script);
        }
        for extension in ["exe", "cmd", "bat"] {
            let candidate = directory.join(format!("{script}.{extension}"));
            if candidate.is_file() {
                return candidate;
            }
        }
        directory.join(format!("{script}.exe"))
    }

    pub(crate) fn checked(&self, command: &mut Command) -> Result<(), InstallError> {
        let printable = printable_command(command);
        self.note(format!("$ {printable}"));
        let status = command
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|error| InstallError::io(format!("unable to start `{printable}`"), error))?;
        if status.success() {
            Ok(())
        } else {
            Err(InstallError::Command {
                command: printable,
                status: status.code(),
            })
        }
    }

    pub(crate) fn succeeds(&self, command: &mut Command) -> bool {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    pub(crate) fn capture(&self, command: &mut Command) -> Result<Output, InstallError> {
        let printable = printable_command(command);
        let output = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|error| InstallError::io(format!("unable to start `{printable}`"), error))?;
        if output.status.success() {
            Ok(output)
        } else {
            Err(InstallError::Command {
                command: printable,
                status: output.status.code(),
            })
        }
    }

    pub(crate) fn create_venv(
        &mut self,
        slug: &str,
        python_version: &str,
    ) -> Result<(), InstallError> {
        let uv = self.ensure_uv()?;
        let target = self.venv_dir(slug);
        self.step(format!(
            "Creating {} with uv-managed Python {python_version}",
            target.display()
        ));
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                InstallError::io(format!("unable to create {}", parent.display()), error)
            })?;
        }

        let mut command = uv_venv_command(&uv, python_version, &target);
        scrub_python_environment(&mut command);
        self.checked(&mut command)?;

        let python = self.venv_python(slug);
        if !python.is_file() {
            return Err(InstallError::InvalidConfiguration(format!(
                "uv completed, but {} does not contain an interpreter",
                target.display()
            )));
        }
        let mut version = Command::new(&python);
        version.arg("--version");
        self.checked(&mut version)
    }

    pub(crate) fn pip_install(
        &mut self,
        slug: &str,
        requirements: &[&str],
        options: PipOptions<'_>,
    ) -> Result<(), InstallError> {
        if requirements.is_empty() {
            return Ok(());
        }
        let uv = self.ensure_uv()?;
        let python = self.venv_python(slug);
        let mut command = Command::new(uv);
        command.args(["pip", "install", "--python"]);
        command.arg(python);
        if options.upgrade {
            command.arg("--upgrade");
        }
        if let Some(index) = options.index_url {
            command.args(["--index-url", index]);
        }
        for index in options.extra_indexes {
            command.args(["--extra-index-url", index]);
        }
        if let Some(strategy) = options.index_strategy {
            command.args(["--index-strategy", strategy]);
        }
        if options.no_build_isolation {
            command.arg("--no-build-isolation");
        }
        command.args(requirements);
        command.envs(options.extra_env.iter().copied());
        scrub_python_environment(&mut command);
        self.checked(&mut command)
    }

    pub(crate) fn install_torch(
        &mut self,
        slug: &str,
        packages: &[&str],
        backend: TorchBackend,
    ) -> Result<(), InstallError> {
        let options = if cfg!(target_os = "macos") {
            PipOptions {
                upgrade: false,
                ..PipOptions::default()
            }
        } else {
            PipOptions {
                index_url: Some(backend.index_url()),
                upgrade: false,
                ..PipOptions::default()
            }
        };
        self.pip_install(slug, packages, options)
    }

    pub(crate) fn select_torch_backend(&mut self) -> Result<TorchBackend, InstallError> {
        if let Some(backend) = self.torch_backend {
            return Ok(backend);
        }

        let requested = self.config.torch_backend;
        if requested == TorchBackendPreference::Cpu {
            self.torch_backend = Some(TorchBackend::Cpu);
            return Ok(TorchBackend::Cpu);
        }
        if cfg!(target_os = "macos") {
            if requested == TorchBackendPreference::Cuda126 {
                return Err(InstallError::InvalidConfiguration(
                    "CUDA is not available on macOS; select the CPU backend".to_owned(),
                ));
            }
            self.note("macOS has no CUDA runtime; selecting CPU PyTorch wheels");
            self.torch_backend = Some(TorchBackend::Cpu);
            return Ok(TorchBackend::Cpu);
        }
        if cfg!(not(target_arch = "x86_64")) {
            if requested == TorchBackendPreference::Cuda126 {
                return Err(InstallError::InvalidConfiguration(
                    "the CUDA 12.6 wheel index is available for x86_64 only".to_owned(),
                ));
            }
            self.note("this architecture has no CUDA 12.6 wheels; selecting CPU");
            self.torch_backend = Some(TorchBackend::Cpu);
            return Ok(TorchBackend::Cpu);
        }

        let minimum = if cfg!(target_os = "windows") {
            "560.76"
        } else {
            "560.28.03"
        };
        let driver = self.nvidia_driver_version();
        let backend = if driver
            .as_deref()
            .is_some_and(|version| version_at_least(version, minimum))
        {
            self.note(format!(
                "Detected NVIDIA driver {}; selecting CUDA 12.6",
                driver.as_deref().unwrap_or_default()
            ));
            TorchBackend::Cuda126
        } else {
            if requested == TorchBackendPreference::Cuda126 {
                let detail = driver
                    .map(|version| format!("driver {version} is older than {minimum}"))
                    .unwrap_or_else(|| "no usable NVIDIA GPU was found".to_owned());
                return Err(InstallError::InvalidConfiguration(format!(
                    "CUDA 12.6 was requested, but {detail}"
                )));
            }
            self.note("No CUDA 12.6-compatible NVIDIA driver was found; selecting CPU");
            TorchBackend::Cpu
        };
        self.torch_backend = Some(backend);
        Ok(backend)
    }

    fn nvidia_driver_version(&self) -> Option<String> {
        let executable = find_nvidia_smi()?;
        let mut list = Command::new(&executable);
        list.arg("-L");
        if !self.succeeds(&mut list) {
            return None;
        }
        let mut query = Command::new(executable);
        query.args(["--query-gpu=driver_version", "--format=csv,noheader"]);
        let output = self.capture(&mut query).ok()?;
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(str::to_owned)
    }

    pub(crate) fn torch_cuda_works(&self, slug: &str) -> bool {
        let mut command = Command::new(self.venv_python(slug));
        command.args([
            "-c",
            "import torch; assert torch.cuda.is_available(); torch.zeros(1, device='cuda')",
        ]);
        scrub_python_environment(&mut command);
        self.succeeds(&mut command)
    }

    pub(crate) fn ensure_uv(&mut self) -> Result<PathBuf, InstallError> {
        if let Some(path) = &self.uv {
            return Ok(path.clone());
        }

        let managed = self
            .config
            .layout
            .environments_root
            .join("uv-bin")
            .join(executable_name("uv"));
        let fallback =
            home_dir().map(|home| home.join(".local").join("bin").join(executable_name("uv")));
        let candidates = self
            .config
            .uv_executable
            .clone()
            .into_iter()
            .chain([managed.clone(), PathBuf::from(executable_name("uv"))])
            .chain(fallback);
        for candidate in candidates {
            let mut probe = Command::new(&candidate);
            probe.arg("--version");
            if self.succeeds(&mut probe) {
                self.uv = Some(candidate.clone());
                return Ok(candidate);
            }
        }

        self.step("Installing uv with Astral's standalone installer");
        let uv_directory = managed.parent().expect("managed uv has a parent");
        fs::create_dir_all(uv_directory).map_err(|error| {
            InstallError::io(
                format!("unable to create {}", uv_directory.display()),
                error,
            )
        })?;
        let scratch = ScratchDir::new_in(uv_directory, "uv-bootstrap")?;
        if cfg!(target_os = "windows") {
            let script = scratch.path().join("install.ps1");
            self.download("https://astral.sh/uv/install.ps1", &script)?;
            let mut command = Command::new("powershell.exe");
            command.args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ]);
            command.arg(script);
            command.env("UV_UNMANAGED_INSTALL", uv_directory);
            self.checked(&mut command)?;
        } else {
            let script = scratch.path().join("install.sh");
            self.download("https://astral.sh/uv/install.sh", &script)?;
            let mut command = Command::new("sh");
            command.arg(script);
            command.env("UV_UNMANAGED_INSTALL", uv_directory);
            self.checked(&mut command)?;
        }
        let mut probe = Command::new(&managed);
        probe.arg("--version");
        if !self.succeeds(&mut probe) {
            return Err(InstallError::InvalidConfiguration(format!(
                "Astral's installer completed, but uv was not found at {}",
                managed.display()
            )));
        }
        self.uv = Some(managed.clone());
        Ok(managed)
    }

    /// Shared package cache for every managed environment. Environments hardlink out of it, so all
    /// micromamba invocations must agree on the location or each one redownloads its packages.
    /// Without it micromamba falls back to `~/micromamba`, outside the layout we manage.
    pub(crate) fn micromamba_root(&self) -> PathBuf {
        self.config.layout.environments_root.join("micromamba")
    }

    pub(crate) fn ensure_micromamba(&mut self) -> Result<PathBuf, InstallError> {
        if let Some(path) = &self.micromamba {
            return Ok(path.clone());
        }
        let managed = self
            .config
            .layout
            .environments_root
            .join("micromamba-bin")
            .join(executable_name("micromamba"));
        let candidates = self
            .config
            .micromamba_executable
            .clone()
            .into_iter()
            .chain([
                managed.clone(),
                PathBuf::from(executable_name("micromamba")),
            ]);
        for candidate in candidates {
            let mut probe = Command::new(&candidate);
            probe.arg("--version");
            if self.succeeds(&mut probe) {
                self.micromamba = Some(candidate.clone());
                return Ok(candidate);
            }
        }

        let url = micromamba_download_url()?;
        self.step(format!("Installing micromamba into {}", managed.display()));
        // A managed binary that exists but failed the probe above is broken; `download` would
        // otherwise keep it on the strength of its size alone.
        if managed.exists() {
            fs::remove_file(&managed).map_err(|error| {
                InstallError::io(
                    format!(
                        "unable to replace the broken micromamba at {}",
                        managed.display()
                    ),
                    error,
                )
            })?;
        }
        self.download(&url, &managed)?;
        make_executable(&managed)?;

        let mut probe = Command::new(&managed);
        probe.arg("--version");
        if !self.succeeds(&mut probe) {
            return Err(InstallError::InvalidConfiguration(format!(
                "the micromamba binary downloaded to {} is not runnable",
                managed.display()
            )));
        }
        self.micromamba = Some(managed.clone());
        Ok(managed)
    }

    /// A micromamba invocation pinned to our own root prefix.
    pub(crate) fn micromamba_command(&mut self) -> Result<Command, InstallError> {
        let micromamba = self.ensure_micromamba()?;
        let root = self.micromamba_root();
        fs::create_dir_all(&root).map_err(|error| {
            InstallError::io(format!("unable to create {}", root.display()), error)
        })?;
        let mut command = Command::new(micromamba);
        command.env("MAMBA_ROOT_PREFIX", &root);
        Ok(command)
    }

    pub(crate) fn reset_mamba_environment(
        &mut self,
        slug: &str,
        python_version: &str,
    ) -> Result<PathBuf, InstallError> {
        let prefix = self.venv_dir(slug);
        let mut remove = self.micromamba_command()?;
        remove
            .args(["env", "remove", "--yes", "--prefix"])
            .arg(&prefix);
        let _ = self.succeeds(&mut remove);
        // `env remove` declines prefixes it did not create, which would leave `create` layering a
        // new environment over the old one instead of resetting it.
        if prefix.exists() {
            fs::remove_dir_all(&prefix).map_err(|error| {
                InstallError::io(
                    format!("unable to clear the environment at {}", prefix.display()),
                    error,
                )
            })?;
        }

        let mut create = self.micromamba_command()?;
        create
            .args(["create", "--yes", "--prefix"])
            .arg(&prefix)
            .args(["-c", CONDA_FORGE])
            .arg(format!("python={python_version}"));
        self.checked(&mut create)?;
        Ok(prefix)
    }

    pub(crate) fn mamba_run(
        &mut self,
        prefix: &Path,
        arguments: &[&str],
    ) -> Result<(), InstallError> {
        let mut command = self.micromamba_command()?;
        command
            .args(["run", "--prefix"])
            .arg(prefix)
            .args(arguments);
        self.checked(&mut command)
    }

    /// Full Conda, needed only by the recipes that delegate to an upstream `install.sh`: those
    /// scripts call `conda info --base`, `conda shell.bash hook`, and `conda activate`, none of
    /// which micromamba provides. Everything we drive ourselves uses [`Self::micromamba_command`].
    pub(crate) fn conda_root(&self) -> PathBuf {
        if let Some(root) = &self.config.conda_root {
            return root.clone();
        }
        let environments = &self.config.layout.environments_root;
        if is_wsl_windows_mount(environments)
            && let Some(home) = home_dir()
        {
            // Miniconda and a number of Conda packages contain Unix symlinks. DrvFS/9P mounts
            // without WSL metadata reject those links, so installing under /mnt/c fails partway
            // through extraction. A stable layout-specific directory avoids both that failure
            // and named-environment collisions between separate applications.
            return home
                .join(".cache")
                .join("bio_tools")
                .join("conda")
                .join(stable_path_id(environments));
        }
        environments.join("conda")
    }

    pub(crate) fn conda_executable_path(&self) -> PathBuf {
        self.config
            .conda_executable
            .clone()
            .unwrap_or_else(|| conda_executable_in(&self.conda_root()))
    }

    pub(crate) fn conda_environment_root(&self) -> PathBuf {
        if self.config.conda_root.is_none()
            && let Some(executable) = &self.config.conda_executable
            && let Some(bin) = executable.parent()
            && matches!(
                bin.file_name().and_then(OsStr::to_str),
                Some("bin" | "Scripts")
            )
            && let Some(root) = bin.parent()
        {
            return root.to_path_buf();
        }
        self.conda_root()
    }

    pub(crate) fn ensure_conda(&mut self) -> Result<PathBuf, InstallError> {
        if let Some(path) = &self.conda {
            return Ok(path.clone());
        }
        let managed_root = self.conda_root();
        let managed = conda_executable_in(&managed_root);
        let candidates = self
            .config
            .conda_executable
            .clone()
            .into_iter()
            .chain([PathBuf::from(executable_name("conda")), managed.clone()]);
        for candidate in candidates {
            let mut probe = Command::new(&candidate);
            probe.arg("--version");
            if self.succeeds(&mut probe) {
                self.conda = Some(candidate.clone());
                self.accept_conda_terms(&candidate);
                return Ok(candidate);
            }
        }
        if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            return Err(InstallError::InvalidConfiguration(
                "Conda is required for this tool and could not be found".to_owned(),
            ));
        }

        self.step(format!(
            "Installing Miniconda into {}",
            managed_root.display()
        ));
        if self.config.conda_root.is_none()
            && self.config.conda_executable.is_none()
            && is_wsl_windows_mount(&self.config.layout.environments_root)
        {
            self.note(format!(
                "Using the native filesystem for Conda because {} is a WSL Windows mount",
                self.config.layout.environments_root.display()
            ));
        }
        let scratch = ScratchDir::new_in(&self.config.layout.environments_root, "miniconda")?;
        let installer = scratch.path().join("miniconda.sh");
        self.download(
            "https://repo.anaconda.com/miniconda/Miniconda3-latest-Linux-x86_64.sh",
            &installer,
        )?;
        let repair_existing = managed_root.exists();
        if repair_existing {
            self.note(format!(
                "The managed Conda prefix at {} is unusable; repairing it with Miniconda update mode",
                managed_root.display()
            ));
        }
        let mut command = miniconda_install_command(&installer, &managed_root, repair_existing);
        self.checked(&mut command)?;

        let mut probe = Command::new(&managed);
        probe.arg("--version");
        if !self.succeeds(&mut probe) {
            return Err(InstallError::InvalidConfiguration(format!(
                "Miniconda completed, but conda was not found at {}",
                managed.display()
            )));
        }
        self.conda = Some(managed.clone());
        self.accept_conda_terms(&managed);
        Ok(managed)
    }

    /// Keep the original `<environments>/conda/envs/<name>/bin/...` contract usable when WSL
    /// forces the real Conda tree onto its native filesystem. These are small launchers rather
    /// than symlinks because the inability to create Unix symlinks is why the fallback exists.
    pub(crate) fn install_conda_environment_shims(&self, tool: Tool) -> Result<(), InstallError> {
        if !cfg!(unix) {
            return Ok(());
        }
        let Some(name) = tool.conda_environment() else {
            return Ok(());
        };
        let actual = self.conda_environment_root().join("envs").join(name);
        let compatibility = self
            .config
            .layout
            .environments_root
            .join("conda")
            .join("envs")
            .join(name);
        if actual == compatibility {
            return Ok(());
        }

        let source_bin = actual.join("bin");
        let target_bin = compatibility.join("bin");
        fs::create_dir_all(&target_bin).map_err(|error| {
            InstallError::io(format!("unable to create {}", target_bin.display()), error)
        })?;
        for executable in ["python", tool.console_script()] {
            let source = source_bin.join(executable);
            if !source.is_file() {
                continue;
            }
            let target = target_bin.join(executable);
            let quoted = source.to_string_lossy().replace('\'', "'\"'\"'");
            fs::write(&target, format!("#!/bin/sh\nexec '{quoted}' \"$@\"\n")).map_err(
                |error| InstallError::io(format!("unable to write {}", target.display()), error),
            )?;
            make_executable(&target)?;
        }
        Ok(())
    }

    /// Conda 24.9+ refuses to touch the Anaconda `defaults` channels non-interactively until their
    /// terms are accepted, and the upstream installers we shell out to still resolve against them.
    /// Older versions do not expose `conda tos`, so absence of that subcommand is harmless.
    fn accept_conda_terms(&mut self, conda: &Path) {
        if self.conda_terms_accepted {
            return;
        }
        self.conda_terms_accepted = true;
        for channel in [
            "https://repo.anaconda.com/pkgs/main",
            "https://repo.anaconda.com/pkgs/r",
        ] {
            let mut command = Command::new(conda);
            command.args(["tos", "accept", "--override-channels", "--channel", channel]);
            let _ = self.succeeds(&mut command);
        }
    }

    pub(crate) fn download(&self, url: &str, destination: &Path) -> Result<(), InstallError> {
        if destination
            .metadata()
            .is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
        {
            self.note(format!("Already have {}", destination.display()));
            return Ok(());
        }
        url::Url::parse(url).map_err(|error| InstallError::Download {
            url: url.to_owned(),
            message: error.to_string(),
        })?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                InstallError::io(format!("unable to create {}", parent.display()), error)
            })?;
        }
        self.step(format!("Downloading {}", destination.display()));
        let partial = append_to_path(destination, ".partial");
        let mut response = ureq::get(url)
            .call()
            .map_err(|error| InstallError::Download {
                url: url.to_owned(),
                message: error.to_string(),
            })?;
        let mut file = File::create(&partial).map_err(|error| {
            InstallError::io(format!("unable to create {}", partial.display()), error)
        })?;
        io::copy(&mut response.body_mut().as_reader(), &mut file).map_err(|error| {
            InstallError::io(format!("unable to write {}", partial.display()), error)
        })?;
        file.flush().map_err(|error| {
            InstallError::io(format!("unable to flush {}", partial.display()), error)
        })?;
        drop(file);
        if destination.exists() {
            fs::remove_file(destination).map_err(|error| {
                InstallError::io(
                    format!("unable to replace {}", destination.display()),
                    error,
                )
            })?;
        }
        fs::rename(&partial, destination).map_err(|error| {
            InstallError::io(
                format!(
                    "unable to rename {} to {}",
                    partial.display(),
                    destination.display()
                ),
                error,
            )
        })
    }

    pub(crate) fn clone_or_update(&self, url: &str, target: &Path) -> Result<(), InstallError> {
        if target.join(".git").is_dir() {
            self.step(format!("Updating {}", target.display()));
            let mut fetch = Command::new("git");
            fetch
                .args(["-C"])
                .arg(target)
                .args(["fetch", "--depth", "1", "origin", "HEAD"]);
            self.checked(&mut fetch)?;
            let mut reset = Command::new("git");
            reset
                .args(["-C"])
                .arg(target)
                .args(["reset", "--hard", "FETCH_HEAD"]);
            return self.checked(&mut reset);
        }
        if target.exists() {
            return Err(InstallError::InvalidConfiguration(format!(
                "{} exists but is not a Git checkout; refusing to overwrite it",
                target.display()
            )));
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                InstallError::io(format!("unable to create {}", parent.display()), error)
            })?;
        }
        self.step(format!("Cloning {url} into {}", target.display()));
        let mut clone = Command::new("git");
        clone.args(["clone", "--depth", "1", url]).arg(target);
        self.checked(&mut clone)
    }

    pub(crate) fn extract_archive(
        &self,
        archive: &Path,
        destination: &Path,
    ) -> Result<(), InstallError> {
        fs::create_dir_all(destination).map_err(|error| {
            InstallError::io(format!("unable to create {}", destination.display()), error)
        })?;
        let mut command = Command::new("tar");
        if archive
            .extension()
            .is_some_and(|extension| extension == "gz")
        {
            command.arg("-xzf");
        } else {
            command.arg("-xf");
        }
        command.arg(archive).arg("-C").arg(destination);
        self.checked(&mut command)
    }

    pub(crate) fn install_fetched_python_script(
        &self,
        slug: &str,
        url: &str,
        name: &str,
    ) -> Result<PathBuf, InstallError> {
        let destination = self.venv_script(slug, name);
        let source = append_to_path(&destination, ".source");
        self.download(url, &source)?;
        let mut source_bytes = Vec::new();
        File::open(&source)
            .and_then(|mut file| file.read_to_end(&mut source_bytes))
            .map_err(|error| {
                InstallError::io(format!("unable to read {}", source.display()), error)
            })?;
        let partial = append_to_path(&destination, ".partial");
        let mut file = File::create(&partial).map_err(|error| {
            InstallError::io(format!("unable to create {}", partial.display()), error)
        })?;
        writeln!(file, "#!{}", self.venv_python(slug).display()).map_err(|error| {
            InstallError::io(format!("unable to write {}", partial.display()), error)
        })?;
        file.write_all(&source_bytes).map_err(|error| {
            InstallError::io(format!("unable to write {}", partial.display()), error)
        })?;
        drop(file);
        fs::rename(&partial, &destination).map_err(|error| {
            InstallError::io(
                format!("unable to install {}", destination.display()),
                error,
            )
        })?;
        let _ = fs::remove_file(source);
        make_executable(&destination)?;
        Ok(destination)
    }

    pub(crate) fn support_file(&self, relative: &str) -> Option<PathBuf> {
        let root = self.config.support_root.as_ref()?;
        let candidate = root.join(relative);
        candidate.is_file().then_some(candidate)
    }

    pub(crate) fn run_upstream_script(
        &self,
        script: &Path,
        arguments: &[&str],
        cwd: &Path,
    ) -> Result<(), InstallError> {
        let mut command = Command::new("bash");
        command.arg(script).args(arguments).current_dir(cwd);
        self.checked(&mut command)
    }
}

pub(crate) struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    pub(crate) fn new_in(parent: &Path, prefix: &str) -> Result<Self, InstallError> {
        fs::create_dir_all(parent).map_err(|error| {
            InstallError::io(format!("unable to create {}", parent.display()), error)
        })?;
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for _ in 0..100 {
            let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                ".{prefix}-{}-{epoch}-{counter}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(InstallError::io(
                        format!(
                            "unable to create temporary directory under {}",
                            parent.display()
                        ),
                        error,
                    ));
                }
            }
        }
        Err(InstallError::InvalidConfiguration(format!(
            "unable to allocate a temporary directory under {}",
            parent.display()
        )))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub(crate) fn directory_contains_extension(root: &Path, extensions: &[&str]) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        if path.is_dir() {
            directory_contains_extension(&path, extensions)
        } else {
            path.extension()
                .and_then(OsStr::to_str)
                .is_some_and(|extension| extensions.contains(&extension))
        }
    })
}

pub(crate) fn scrub_python_environment(command: &mut Command) {
    for variable in PYTHON_ENVIRONMENT_VARIABLES {
        command.env_remove(variable);
    }
}

fn append_to_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn executable_name(name: &str) -> OsString {
    if cfg!(target_os = "windows") {
        format!("{name}.exe").into()
    } else {
        name.into()
    }
}

fn uv_venv_command(uv: &Path, python_version: &str, target: &Path) -> Command {
    let mut command = Command::new(uv);
    command.args([
        "venv",
        "--no-project",
        "--managed-python",
        "--python",
        python_version,
        "--clear",
    ]);
    command.arg(target);
    command
}

/// micromamba publishes a bare statically linked binary per platform, so bootstrapping is a
/// download and a chmod rather than Miniconda's ~500 MB self-extracting base environment.
fn micromamba_download_url() -> Result<String, InstallError> {
    let asset = match (env::consts::OS, env::consts::ARCH) {
        ("linux", "x86_64") => "micromamba-linux-64",
        ("linux", "aarch64") => "micromamba-linux-aarch64",
        ("linux", "powerpc64") => "micromamba-linux-ppc64le",
        ("macos", "x86_64") => "micromamba-osx-64",
        ("macos", "aarch64") => "micromamba-osx-arm64",
        ("windows", "x86_64") => "micromamba-win-64.exe",
        ("windows", "aarch64") => "micromamba-win-arm64.exe",
        (os, arch) => {
            return Err(InstallError::InvalidConfiguration(format!(
                "micromamba does not publish a binary for {os}-{arch}"
            )));
        }
    };
    Ok(format!("{MICROMAMBA_RELEASES}/{asset}"))
}

fn miniconda_install_command(installer: &Path, target: &Path, update: bool) -> Command {
    let mut command = Command::new("bash");
    command.arg(installer).arg("-b");
    if update {
        command.arg("-u");
    }
    command.arg("-p").arg(target);
    command
}

fn conda_executable_in(root: &Path) -> PathBuf {
    root.join(if cfg!(target_os = "windows") {
        "Scripts"
    } else {
        "bin"
    })
    .join(executable_name("conda"))
}

fn home_dir() -> Option<PathBuf> {
    env::var_os(if cfg!(target_os = "windows") {
        "USERPROFILE"
    } else {
        "HOME"
    })
    .map(PathBuf::from)
}

fn is_wsl_windows_mount(path: &Path) -> bool {
    if !cfg!(target_os = "linux")
        || (env::var_os("WSL_DISTRO_NAME").is_none() && env::var_os("WSL_INTEROP").is_none())
    {
        return false;
    }
    let mut components = path.components();
    matches!(components.next(), Some(std::path::Component::RootDir))
        && components
            .next()
            .is_some_and(|part| part.as_os_str() == "mnt")
        && components.next().is_some_and(|part| {
            let drive = part.as_os_str().to_string_lossy();
            drive.len() == 1 && drive.as_bytes()[0].is_ascii_alphabetic()
        })
}

fn stable_path_id(path: &Path) -> String {
    // FNV-1a is sufficient here: this is a stable directory label, not a security boundary.
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in path.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn find_nvidia_smi() -> Option<PathBuf> {
    if let Some(path) = find_on_path("nvidia-smi") {
        return Some(path);
    }
    if cfg!(target_os = "windows") {
        for candidate in [
            env::var_os("SystemRoot")
                .map(PathBuf::from)
                .map(|root| root.join("System32").join("nvidia-smi.exe")),
            env::var_os("ProgramFiles").map(PathBuf::from).map(|root| {
                root.join("NVIDIA Corporation")
                    .join("NVSMI")
                    .join("nvidia-smi.exe")
            }),
        ]
        .into_iter()
        .flatten()
        {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    } else {
        let wsl = PathBuf::from("/usr/lib/wsl/lib/nvidia-smi");
        if wsl.is_file() {
            return Some(wsl);
        }
    }
    None
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    let extensions: Vec<OsString> = if cfg!(target_os = "windows") {
        env::var_os("PATHEXT")
            .map(|value| {
                value
                    .to_string_lossy()
                    .split(';')
                    .map(|extension| extension.to_ascii_lowercase().into())
                    .collect()
            })
            .unwrap_or_else(|| vec![".exe".into(), ".cmd".into(), ".bat".into()])
    } else {
        vec![OsString::new()]
    };
    for directory in env::split_paths(&path) {
        for extension in &extensions {
            let mut filename = OsString::from(name);
            if cfg!(target_os = "windows") && Path::new(name).extension().is_none() {
                filename.push(extension);
            }
            let candidate = directory.join(filename);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn printable_command(command: &Command) -> String {
    std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|part| quote_argument(&part.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote_argument(argument: &str) -> String {
    if argument
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "-._/:=+@".contains(character))
    {
        argument.to_owned()
    } else if cfg!(target_os = "windows") {
        format!("\"{}\"", argument.replace('"', "\\\""))
    } else {
        format!("'{}'", argument.replace('\'', "'\\''"))
    }
}

fn version_at_least(current: &str, minimum: &str) -> bool {
    let parts = |version: &str| {
        version
            .split('.')
            .map(|part| {
                let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
                digits.parse::<u64>().unwrap_or_default()
            })
            .collect::<Vec<_>>()
    };
    let mut current = parts(current);
    let mut minimum = parts(minimum);
    let length = current.len().max(minimum.len());
    current.resize(length, 0);
    minimum.resize(length, 0);
    current >= minimum
}

fn make_executable(path: &Path) -> Result<(), InstallError> {
    #[cfg(not(unix))]
    let _ = path;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|error| {
                InstallError::io(format!("unable to inspect {}", path.display()), error)
            })?
            .permissions();
        permissions.set_mode(permissions.mode() | 0o111);
        fs::set_permissions(path, permissions).map_err(|error| {
            InstallError::io(
                format!("unable to mark {} executable", path.display()),
                error,
            )
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_versions_are_compared_numerically() {
        assert!(version_at_least("560.76", "560.28.03"));
        assert!(version_at_least("560.28.3", "560.28.03"));
        assert!(!version_at_least("559.99", "560.28.03"));
        assert!(!version_at_least("560.27.99", "560.28.03"));
    }

    #[test]
    fn partial_suffix_does_not_replace_a_real_extension() {
        assert_eq!(
            append_to_path(Path::new("weights/model.pt"), ".partial"),
            Path::new("weights/model.pt.partial")
        );
    }

    #[test]
    fn tool_venv_does_not_inherit_the_calling_project() {
        let command = uv_venv_command(
            Path::new("uv"),
            "3.10",
            Path::new("environments/rfantibody"),
        );
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                "venv",
                "--no-project",
                "--managed-python",
                "--python",
                "3.10",
                "--clear",
                "environments/rfantibody",
            ]
        );
    }

    #[test]
    fn micromamba_asset_matches_the_build_platform() {
        let url = micromamba_download_url().expect("this platform has a micromamba binary");
        assert!(url.starts_with(MICROMAMBA_RELEASES), "{url}");
        let asset = url.rsplit('/').next().unwrap_or_default();
        assert_eq!(
            asset.ends_with(".exe"),
            cfg!(target_os = "windows"),
            "only the Windows assets carry an extension: {asset}"
        );
        let platform = if cfg!(target_os = "macos") {
            "osx"
        } else if cfg!(target_os = "windows") {
            "win"
        } else {
            "linux"
        };
        assert!(
            asset.starts_with(&format!("micromamba-{platform}-")),
            "{asset}"
        );
    }

    #[test]
    fn existing_miniconda_prefix_uses_update_mode() {
        let fresh = miniconda_install_command(
            Path::new("miniconda.sh"),
            Path::new("environments/conda"),
            false,
        );
        assert_eq!(
            fresh.get_args().collect::<Vec<_>>(),
            ["miniconda.sh", "-b", "-p", "environments/conda"]
        );

        let repair = miniconda_install_command(
            Path::new("miniconda.sh"),
            Path::new("environments/conda"),
            true,
        );
        assert_eq!(
            repair.get_args().collect::<Vec<_>>(),
            ["miniconda.sh", "-b", "-u", "-p", "environments/conda"]
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn wsl_drive_mount_detection_is_narrow() {
        if env::var_os("WSL_DISTRO_NAME").is_none() && env::var_os("WSL_INTEROP").is_none() {
            return;
        }
        assert!(is_wsl_windows_mount(Path::new("/mnt/c/projects/tools")));
        assert!(!is_wsl_windows_mount(Path::new("/home/user/tools")));
        assert!(!is_wsl_windows_mount(Path::new("/mnt/shared/tools")));
    }

    #[cfg(unix)]
    #[test]
    fn native_conda_environment_gets_compatibility_launchers() {
        let scratch = ScratchDir::new_in(&env::temp_dir(), "conda-shims").unwrap();
        let environments = scratch.path().join("mounted-environments");
        let native_conda = scratch.path().join("native-conda");
        let source = native_conda.join("envs/genie3/bin");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("python"), "python").unwrap();
        fs::write(source.join("genie3"), "genie3").unwrap();

        let mut config = super::super::InstallConfig::new(scratch.path());
        config.layout = super::super::InstallLayout::split(scratch.path(), &environments);
        config.conda_root = Some(native_conda);
        let installer = Installer::from_config(config);
        installer
            .install_conda_environment_shims(Tool::Genie3)
            .unwrap();

        let target = environments.join("conda/envs/genie3/bin");
        assert!(target.join("python").is_file());
        assert!(target.join("genie3").is_file());
        assert!(
            fs::read_to_string(target.join("python"))
                .unwrap()
                .contains("native-conda/envs/genie3/bin/python")
        );
    }
}
