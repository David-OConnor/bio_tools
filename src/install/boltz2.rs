use std::process::Command;

use super::{
    InstallError, Installer,
    common::{PipOptions, TorchBackend},
};
use crate::tool_definitions::Tool;

const TORCH: &[&str] = &["torch==2.7.1"];

pub(super) fn install(installer: &mut Installer) -> Result<(), InstallError> {
    const SLUG: &str = Tool::Boltz2.slug();
    let backend = installer.select_torch_backend()?;
    installer.create_venv(SLUG, "3.12")?;
    installer.install_torch(SLUG, TORCH, backend)?;

    // cuequivariance wheels from the CUDA extra are Linux-only. Plain Boltz still uses a CUDA
    // Torch build and is therefore a functional fallback, not a CPU-only build.
    if backend == TorchBackend::Cuda126 && cfg!(target_os = "linux") {
        if let Err(error) =
            installer.pip_install(SLUG, &["boltz[cuda]~=2.2.1"], PipOptions::default())
        {
            installer.note(format!(
                "The optional Boltz CUDA extra did not resolve ({error}); installing plain Boltz"
            ));
            installer.pip_install(SLUG, &["boltz~=2.2.1"], PipOptions::default())?;
        }
    } else {
        installer.pip_install(SLUG, &["boltz~=2.2.1"], PipOptions::default())?;
    }

    // Boltz depends on Torch, so installing it can replace the pinned build with whatever PyPI
    // serves. On Windows that is a CPU-only wheel, which leaves Boltz (default `--accelerator
    // gpu`) failing at run time with "No supported gpu backend found". Re-applying the pin is a
    // no-op when the build survived, and restores it when it did not.
    installer.install_torch(SLUG, TORCH, backend)?;

    let executable = installer.venv_script(SLUG, Tool::Boltz2.console_script());
    let mut verify = Command::new(&executable);
    verify.arg("--help");
    installer.checked(&mut verify)?;
    if backend == TorchBackend::Cuda126 && !installer.torch_cuda_works(SLUG) {
        return Err(InstallError::InvalidConfiguration(
            "Boltz-2's Torch cannot reach the GPU, although a CUDA 12.6-compatible NVIDIA driver \
             was detected; Boltz defaults to the GPU accelerator and would fail at run time"
                .to_owned(),
        ));
    }
    installer.note("Boltz model weights download on first use");
    Ok(())
}
