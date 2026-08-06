use std::{env, fs, path::PathBuf, process::Command};

use super::{
    InstallError, Installer,
    common::{PipOptions, ScratchDir, TorchBackend, scrub_python_environment},
};
use crate::tool_definitions::Tool;

const SLUG: &str = Tool::OpenDde.slug();

pub(super) fn install(installer: &mut Installer) -> Result<(), InstallError> {
    let mut backend = installer.select_torch_backend()?;
    installer.create_venv(SLUG, "3.13")?;

    let mut installed = install_backend(installer, backend);
    if installed.is_ok() && backend == TorchBackend::Cuda126 {
        let mut verify_cuda = Command::new(installer.venv_python(SLUG));
        verify_cuda.args([
            "-c",
            "import torch; assert torch.cuda.is_available() and torch.version.cuda and \
             torch.version.cuda.startswith('12.6'); torch.zeros(1, device='cuda')",
        ]);
        scrub_python_environment(&mut verify_cuda);
        if !installer.succeeds(&mut verify_cuda) {
            installed = Err(InstallError::InvalidConfiguration(
                "the OpenDDE CUDA runtime could not allocate a GPU tensor".to_owned(),
            ));
        }
    }
    if let Err(error) = installed {
        if backend != TorchBackend::Cuda126 {
            return Err(error);
        }
        installer.note(format!(
            "CUDA installation or verification failed ({error}); rebuilding OpenDDE for CPU"
        ));
        backend = TorchBackend::Cpu;
        installer.create_venv(SLUG, "3.13")?;
        install_backend(installer, backend)?;
    }

    let executable = installer.venv_script(SLUG, Tool::OpenDde.console_script());
    let mut version = Command::new(&executable);
    version.arg("--version");
    installer.checked(&mut version)?;
    let mut doctor = Command::new(&executable);
    doctor.arg("doctor");
    installer.checked(&mut doctor)?;

    if installer.config.prewarm_opendde {
        prewarm(installer, &executable)?;
    }
    installer.note(format!(
        "OpenDDE installed with the {} backend",
        backend.label()
    ));
    Ok(())
}

fn install_backend(installer: &mut Installer, backend: TorchBackend) -> Result<(), InstallError> {
    installer.step(format!(
        "Installing OpenDDE with the {} backend",
        backend.label()
    ));
    installer.install_torch(
        SLUG,
        &["torch==2.7.1", "torchvision==0.22.1", "torchaudio==2.7.1"],
        backend,
    )?;
    let package = if backend == TorchBackend::Cuda126 {
        "opendde[gpu]~=1.0.2"
    } else {
        "opendde~=1.0.2"
    };
    installer.pip_install(SLUG, &[package], PipOptions::default())
}

fn prewarm(installer: &Installer, executable: &PathBuf) -> Result<(), InstallError> {
    let root = installer.config.opendde_root.clone().or_else(|| {
        env::var_os(if cfg!(target_os = "windows") {
            "USERPROFILE"
        } else {
            "HOME"
        })
        .map(PathBuf::from)
        .map(|home| home.join(".cache").join("opendde"))
    });
    if root
        .as_ref()
        .is_some_and(|root| root.join("checkpoint").is_dir())
    {
        installer.note("OpenDDE model data is already present");
        return Ok(());
    }

    installer.step("Fetching the OpenDDE model checkpoint (several GB)");
    let scratch = ScratchDir::new_in(
        &installer.config.layout.environments_root,
        "opendde-prewarm",
    )?;
    fs::write(
        scratch.path().join("input.json"),
        r#"[{"name":"prewarm","modelSeeds":[101],"sequences":[{"proteinChain":{"sequence":"ACDEFG","count":1,"id":["A"]}}]}]"#,
    )
    .map_err(|error| InstallError::io("unable to write OpenDDE prewarm input", error))?;
    let mut command = Command::new(executable);
    command
        .args([
            "pred",
            "-i",
            "input.json",
            "-o",
            "output",
            "-n",
            "opendde_v1",
            "--use_msa",
            "false",
            "--use_template",
            "false",
            "--use_rna_msa",
            "false",
            "--sample",
            "1",
            "--step",
            "1",
            "--cycle",
            "1",
        ])
        .current_dir(scratch.path());
    if installer.succeeds(&mut command) {
        installer.note("OpenDDE model data cached; the first prediction can start immediately");
    } else {
        installer.note("Could not pre-fetch OpenDDE model data; it will download on first use");
    }
    Ok(())
}
