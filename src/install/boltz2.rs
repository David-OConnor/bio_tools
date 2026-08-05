use std::process::Command;

use super::{
    InstallError, Installer,
    common::{PipOptions, TorchBackend},
};

pub(super) fn install(installer: &mut Installer) -> Result<(), InstallError> {
    const SLUG: &str = "boltz2";
    let backend = installer.select_torch_backend()?;
    installer.create_venv(SLUG, "3.12")?;
    installer.install_torch(SLUG, &["torch==2.7.1"], backend)?;

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

    let executable = installer.venv_script(SLUG, "boltz");
    let mut verify = Command::new(&executable);
    verify.arg("--help");
    installer.checked(&mut verify)?;
    if backend == TorchBackend::Cuda126 && !installer.torch_cuda_works(SLUG) {
        installer.note("Warning: Torch cannot reach the GPU; Boltz will run on CPU");
    }
    installer.note("Boltz model weights download on first use");
    Ok(())
}
