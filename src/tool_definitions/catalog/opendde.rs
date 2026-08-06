use super::{CatalogEntry, Identity};
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense, tool_definitions::Tool};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::OpenDde),
    categories: &[
        ProcessCategory::StructurePrediction,
        ProcessCategory::ProteinDesign,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: true,
    summary: "Run a protein, DNA/RNA, or ligand co-folding prediction.",
    description: "Predict protein structure from nucleic acid or amino acid sequence. Predict the structure of ligands, nucleic acids, and ions, or complexes of these molecules. This is a relatively new, but very accurate model.",
    availability: "Installed by setup_system.sh into its own uv environment; model assets download separately",
    license_details: "Apache 2.0 (Aureka Research). Commercial use is permitted, with the licence's attribution and notice conditions.",
    repo_url: Some("https://github.com/aurekaresearch/OpenDDE"),
    home_url: Some("https://aurekaresearch.github.io/OpenDDE-Website/"),
    docs_url: Some("https://github.com/aurekaresearch/OpenDDE/blob/main/docs/tutorial.md"),
};
