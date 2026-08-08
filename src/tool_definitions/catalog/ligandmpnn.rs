use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::LigandMpnn),
    categories: &[
        ToolCategory::SequencePrediction,
        ToolCategory::ProteinDesign,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    spec: SpecData {
        summary: "Design protein sequences with ligand-, solubility-, or membrane-aware models.",
        description: "Runs the official LigandMPNN run.py, ProteinMPNN's successor CLI covering protein, ligand-aware, soluble-only, and membrane-topology model types behind a single interface.",
        availability: "Installed by setup_system.sh with CUDA-enabled PyTorch for Linux/WSL and an NVIDIA GPU",
        license_details: "MIT, weights included. Commercial use is unrestricted.",
        repo_url: Some("https://github.com/dauparas/LigandMPNN"),
        home_url: None,
        docs_url: None,
        paper_url: Some("https://www.biorxiv.org/content/10.1101/2023.12.22.573103v1"),
        license: License::Mit,
        license_url: None,
    },
};
