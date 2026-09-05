use std::{path::Path, process::Command};

use super::{
    InstallError, Installer,
    common::{PipOptions, TorchBackend},
};
use crate::tool_definitions::Tool;

const LIGAND_WEIGHTS_ROOT: &str = "https://files.ipd.uw.edu/pub/ligandmpnn";
const ABMPNN_WEIGHTS: &str = "https://zenodo.org/records/8164693/files/abmpnn.pt?download=1";

// The public variants listed by LigandMPNN's get_model_params.sh. Keep the
// catalog checkpoint selectors and installation/status checks in agreement.
pub(crate) const LIGAND_CHECKPOINTS: &[&str] = &[
    "proteinmpnn_v_48_002.pt",
    "proteinmpnn_v_48_010.pt",
    "proteinmpnn_v_48_020.pt",
    "proteinmpnn_v_48_030.pt",
    "ligandmpnn_v_32_005_25.pt",
    "ligandmpnn_v_32_010_25.pt",
    "ligandmpnn_v_32_020_25.pt",
    "ligandmpnn_v_32_030_25.pt",
    "solublempnn_v_48_002.pt",
    "solublempnn_v_48_010.pt",
    "solublempnn_v_48_020.pt",
    "solublempnn_v_48_030.pt",
    "per_residue_label_membrane_mpnn_v_48_020.pt",
    "global_label_membrane_mpnn_v_48_020.pt",
    "ligandmpnn_sc_v_32_002_16.pt",
];

pub(super) fn install_ligand(installer: &mut Installer) -> Result<(), InstallError> {
    const SLUG: &str = Tool::LigandMpnn.slug();
    install_runtime(installer, SLUG)?;
    let target = installer.tools_root().join("LigandMPNN");
    installer.clone_or_update("https://github.com/dauparas/LigandMPNN", &target)?;

    // run.py imports ProDy and the bundled OpenFold packing helpers even
    // when side-chain packing is disabled. Do not replace the selected Torch
    // backend with the upstream requirements file's older CUDA-only pins.
    installer.pip_install(
        SLUG,
        &[
            "numpy==1.26.4",
            "ProDy==2.6.1",
            "scipy==1.12.0",
            "biopython==1.83",
            "ml-collections==0.1.1",
            "dm-tree==0.1.8",
        ],
        PipOptions::default(),
    )?;
    // Bundled OpenFold uses the removed np.int alias in three dtype
    // declarations. Use explicit 64-bit indices, as required by Torch's
    // one_hot, allowing Python 3.12 / NumPy 1.26 without changing calculations.
    let constants = target.join("openfold/np/residue_constants.py");
    let source = std::fs::read_to_string(&constants)
        .map_err(|error| InstallError::io("reading LigandMPNN's OpenFold constants", error))?;
    let compatible = source.replace("dtype=np.int)", "dtype=np.int64)");
    if source != compatible {
        std::fs::write(&constants, compatible).map_err(|error| {
            InstallError::io("updating LigandMPNN's OpenFold integer dtypes", error)
        })?;
    }
    for filename in LIGAND_CHECKPOINTS {
        installer.download(
            &format!("{LIGAND_WEIGHTS_ROOT}/{filename}"),
            &target.join("model_params").join(filename),
        )?;
        require_nonempty(
            &target.join("model_params").join(filename),
            "LigandMPNN model weights did not download",
        )?;
    }
    let mut probe = Command::new(installer.venv_python(SLUG));
    probe.arg(target.join("run.py")).arg("--help");
    installer.checked(&mut probe)?;
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
