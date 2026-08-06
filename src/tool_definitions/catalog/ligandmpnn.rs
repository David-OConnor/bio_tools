use super::CatalogEntry;
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense};

pub const ENTRY: CatalogEntry = CatalogEntry {
    slug: "ligandmpnn",
    name: "LigandMPNN",
    categories: &[
        ProcessCategory::SequencePrediction,
        ProcessCategory::ProteinDesign,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    summary: "Design protein sequences with ligand-, solubility-, or membrane-aware models.",
    description: "Runs the official LigandMPNN run.py, ProteinMPNN's successor CLI covering protein, ligand-aware, soluble-only, and membrane-topology model types behind a single interface.",
    availability: "Installed by setup_system.sh with CUDA-enabled PyTorch for Linux/WSL and an NVIDIA GPU",
    license_details: "MIT, weights included. Commercial use is unrestricted.",
    repo_url: Some("https://github.com/dauparas/LigandMPNN"),
    home_url: None,
    docs_url: None,
};
