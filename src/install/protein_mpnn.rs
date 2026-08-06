use std::{path::Path, process::Command};

use super::{
    InstallError, Installer,
    common::{PipOptions, TorchBackend},
};
use crate::tool_definitions::Tool;

const LIGAND_WEIGHTS_ROOT: &str = "https://files.ipd.uw.edu/pub/ligandmpnn";
const ABMPNN_WEIGHTS: &str = "https://zenodo.org/records/8164693/files/abmpnn.pt?download=1";

pub(super) fn install_ligand(installer: &mut Installer) -> Result<(), InstallError> {
    const SLUG: &str = Tool::LigandMpnn.slug();
    install_runtime(installer, SLUG)?;
    let target = installer.tools_root().join("LigandMPNN");
    installer.clone_or_update("https://github.com/dauparas/LigandMPNN", &target)?;

    for filename in [
        "ligandmpnn_v_32_010_25.pt",
        "proteinmpnn_v_48_020.pt",
        "solublempnn_v_48_020.pt",
    ] {
        installer.download(
            &format!("{LIGAND_WEIGHTS_ROOT}/{filename}"),
            &target.join("model_params").join(filename),
        )?;
    }
    require_nonempty(
        &target.join("model_params/ligandmpnn_v_32_010_25.pt"),
        "the LigandMPNN weights did not download",
    )?;
    installer.note(format!("LigandMPNN installed at {}", target.display()));
    Ok(())
}

pub(super) fn install_protein(installer: &mut Installer) -> Result<(), InstallError> {
    const SLUG: &str = Tool::ProteinMpnn.slug();
    install_runtime(installer, SLUG)?;
    let target = installer.tools_root().join("ProteinMPNN");
    installer.clone_or_update("https://github.com/dauparas/ProteinMPNN", &target)?;
    require_nonempty(
        &target.join("vanilla_model_weights/v_48_020.pt"),
        "the ProteinMPNN checkout does not contain its vanilla model weights",
    )?;
    installer.download(ABMPNN_WEIGHTS, &target.join("abmpnn_weights/v_48_020.pt"))?;
    convert_weights(installer, &target);
    installer.note(format!("ProteinMPNN installed at {}", target.display()));
    Ok(())
}

fn install_runtime(installer: &mut Installer, slug: &str) -> Result<(), InstallError> {
    let backend = installer.select_torch_backend()?;
    installer.create_venv(slug, "3.12")?;
    installer.install_torch(slug, &["torch==2.7.1"], backend)?;
    installer.pip_install(slug, &["numpy<2"], PipOptions::default())?;
    if backend == TorchBackend::Cuda126 && !installer.torch_cuda_works(slug) {
        installer.note("Warning: Torch cannot reach the GPU; MPNN will run on CPU");
    }
    Ok(())
}

fn convert_weights(installer: &Installer, target: &Path) {
    let converter = [
        "scripts/convert_mpnn_weights.py",
        "install_scripts/convert_mpnn_weights.py",
        "convert_mpnn_weights.py",
    ]
    .into_iter()
    .find_map(|candidate| installer.support_file(candidate));
    let Some(converter) = converter else {
        installer.note(
            "convert_mpnn_weights.py was not supplied; skipping native ddG weight conversion",
        );
        return;
    };

    installer.step("Converting ProteinMPNN weights for the native ddG scanner");
    let mut command = Command::new(installer.venv_python(Tool::ProteinMpnn.slug()));
    command
        .arg(converter)
        .arg("--checkpoint")
        .arg(target.join("vanilla_model_weights/v_48_020.pt"))
        .arg("--output")
        .arg(target.join("converted/v_48_020.mcnn"))
        .arg("--repo")
        .arg(target);
    if installer.succeeds(&mut command) {
        installer.note("Native ddG scanning weights are available");
    } else {
        installer.note("Weight conversion failed; Python MPNN remains installed and usable");
    }
}

fn require_nonempty(path: &Path, message: &str) -> Result<(), InstallError> {
    if path.metadata().is_ok_and(|metadata| metadata.len() > 0) {
        Ok(())
    } else {
        Err(InstallError::InvalidConfiguration(format!(
            "{message}: {}",
            path.display()
        )))
    }
}
