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
        summary: "Atomic context-conditioned protein sequence design",
        description: "A deep learning-based protein sequence design method that explicitly \
        models all non-protein components of biomolecular systems. \
        LigandMPNN generates not only sequences but also sidechain conformations to allow detailed \
        evaluation of binding interactions. Experimental characterization demonstrates that LigandMPNN can \
        generate small molecule and DNA-binding proteins with high affinity and specificity. \
        It allows explicit modeling of small molecule, nucleotide, metal, and other atomic contexts.",
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
