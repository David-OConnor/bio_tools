use std::{fs, path::Path, process::Command};

use super::{
    InstallError, Installer, TorchBackendPreference,
    common::{CONDA_FORGE, PipOptions, TorchBackend, version_at_least},
    first_env,
};
use crate::{run::CommandSpec, tool_definitions::Tool};

const TOOL: Tool = Tool::OpenFold3;
const SLUG: &str = TOOL.slug();
const REQUIREMENT: &str = "openfold3==0.5.1";
const CHECKPOINT_URL: &str = concat!(
    "https://openfold3-data.s3.amazonaws.com/openfold3-parameters/",
    "of3-ob-2025-06-30-174k.pt",
);
pub(crate) const CHECKPOINT: &str = "of3-ob-2025-06-30-174k.pt";
const DEVICE_PROBE: &str =
    include_str!("../adapters/python/bio_tool_adapters/tool_scripts/openfold3_device.py");

pub(crate) fn device_command(installer: &Installer) -> CommandSpec {
    installer
        .tool_python_command(TOOL)
        .args(["-I", "-c", DEVICE_PROBE])
}

fn requested_backend(installer: &Installer) -> Result<String, InstallError> {
    let requested = first_env(&["OPENFOLD3_BACKEND"])
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_else(|| {
            match installer.config.torch_backend {
                TorchBackendPreference::Auto => "auto",
                TorchBackendPreference::Cpu => "cpu",
                TorchBackendPreference::Cuda126 => "cuda",
            }
            .to_owned()
        });

    let supported = match requested.as_str() {
        "auto" | "cpu" => true,
        "cuda" => cfg!(target_os = "linux"),
        "rocm" => cfg!(all(target_os = "linux", target_arch = "x86_64")),
        "mps" => cfg!(all(target_os = "macos", target_arch = "aarch64")),
        _ => {
            return Err(InstallError::InvalidConfiguration(
                "OPENFOLD3_BACKEND must be auto, cpu, cuda, rocm, or mps".to_owned(),
            ));
        }
    };
    if !supported {
        return Err(InstallError::InvalidConfiguration(format!(
            "OpenFold3 cannot install the {requested} backend on this platform"
        )));
    }

    Ok(requested)
}

fn install_runtime(
    installer: &mut Installer,
    backend: TorchBackend,
    probe: &str,
) -> Result<(), InstallError> {
    let label = if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "Apple Silicon"
    } else {
        backend.label()
    };
    installer.step(format!("Installing OpenFold3 with {label} PyTorch"));

    if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        // Current PyPI PyTorch wheels for macOS are arm64-only.
        let prefix = installer.reset_mamba_environment(SLUG, "3.12")?;
        let mut command = installer.micromamba_command()?;
        command
            .args(["install", "--yes", "--prefix"])
            .arg(prefix)
            .args(["-c", CONDA_FORGE, "pytorch=2.10.0=cpu_generic*"]);
        installer.checked(&mut command)?;
    } else {
        installer.create_venv(SLUG, "3.12")?;
        let torch = if backend == TorchBackend::Rocm72 {
            "torch==2.11.0"
        } else {
            "torch==2.10.0"
        };
        installer.install_torch(SLUG, &[torch], backend)?;
    }

    installer.pip_install(
        SLUG,
        &[REQUIREMENT],
        PipOptions {
            upgrade: false,
            ..PipOptions::default()
        },
    )?;

    let mut verify = Command::new(installer.venv_python(SLUG));
    verify.args(["-I", "-c", DEVICE_PROBE, probe]);
    installer.checked(&mut verify)?;

    let mut cli = Command::new(installer.executable_path(TOOL));
    cli.args(["predict", "--help"]);
    installer.checked(&mut cli)
}

pub(super) fn install(installer: &mut Installer) -> Result<(), InstallError> {
    let requested = requested_backend(installer)?;
    let mut backends = Vec::new();
    if cfg!(target_os = "linux") && matches!(requested.as_str(), "auto" | "cuda") {
        if let Some(driver) = installer.nvidia_driver_version() {
            if version_at_least(&driver, "570.26")
                && installer.config.torch_backend != TorchBackendPreference::Cuda126
            {
                backends.push((TorchBackend::Cuda128, "cuda"));
            }
            if version_at_least(&driver, "560.28.03") {
                backends.push((TorchBackend::Cuda126, "cuda"));
            }
        }
        if requested == "cuda" && backends.is_empty() {
            return Err(InstallError::InvalidConfiguration(
                "OpenFold3 needs an NVIDIA driver compatible with CUDA 12.6 or newer".to_owned(),
            ));
        }
    }
    if requested == "rocm"
        || (requested == "auto"
            && cfg!(all(target_os = "linux", target_arch = "x86_64"))
            && Path::new("/dev/kfd").exists())
    {
        backends.push((TorchBackend::Rocm72, "rocm"));
    }
    if matches!(requested.as_str(), "auto" | "cpu" | "mps") {
        backends.push((TorchBackend::Cpu, requested.as_str()));
    }

    let mut backends = backends.into_iter().peekable();
    while let Some((backend, probe)) = backends.next() {
        let Err(error) = install_runtime(installer, backend, probe) else {
            break;
        };
        let Some((next, _)) = backends.peek() else {
            return Err(error);
        };
        installer.note(format!(
            "{} setup failed ({error}); trying {}",
            backend.label(),
            next.label(),
        ));
    }

    let cache = installer
        .tool_cache_dir(TOOL)
        .expect("OpenFold3 has a model cache");
    installer.download(CHECKPOINT_URL, &cache.join(CHECKPOINT))?;

    // CPU and MPS share the same wheel on Apple Silicon. Remember an explicit CPU selection.
    let setting = installer.venv_dir(SLUG).join("openfold3-backend");
    fs::write(&setting, format!("{requested}\n"))
        .map_err(|error| InstallError::io(format!("unable to write {}", setting.display()), error))
}
