use super::CatalogEntry;
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense};

pub const ENTRY: CatalogEntry = CatalogEntry {
    slug: "proteinmpnn",
    name: "ProteinMPNN",
    categories: &[
        ProcessCategory::SequencePrediction,
        ProcessCategory::ProteinDesign,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: true,
    summary: "Design protein sequences for a supplied backbone structure.",
    description: "Runs the official ProteinMPNN fixed-backbone sequence-design script on selected PDB chains.",
    availability: "Installed by setup_system.sh with CUDA-enabled PyTorch for Linux/WSL and an NVIDIA GPU",
    license_details: "MIT, weights included. Commercial use is unrestricted.",
    repo_url: Some("https://github.com/dauparas/ProteinMPNN"),
    home_url: None,
    docs_url: None,
};
