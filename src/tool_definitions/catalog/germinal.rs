use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::Germinal),
    categories: &[
        ToolCategory::AntibodyDesign,
        ToolCategory::ProteinDesign,
        ToolCategory::PeptideBinderDesign,
    ],
    launch_type: LaunchType::CondaBasedApp,
    license_type: LicenseCategory::NonCommercial,
    expense: ProcessExpense::Expensive,
    top_choice: true,
    spec: SpecData {
        summary: "Efficient generation of epitope-targeted de novo antibodies",
        description: "Germinal is a pipeline for designing de novo antibodies against specified epitopes on target proteins. \
        The pipeline follows a 3-step process: hallucination based on ColabDesign, selective sequence redesign with AbMPNN, and \
        cofolding with a structure prediction model. Germinal is capable of designing both nanobodies and scFvs against \
        user-specified residues on target proteins.",
        availability: "Conda environment, an NVIDIA GPU, PyRosetta, and the AlphaFold 2 parameters are required",
        license_details: "MIT code, but the pipeline as it runs is narrower than that: PyRosetta needs a separate licence for commercial use, and the IgLM weights that guide design are released for non-commercial research only.",
        repo_url: Some("https://github.com/SantiagoMille/germinal"),
        home_url: None,
        docs_url: Some("https://www.biorxiv.org/content/10.1101/2025.09.19.677421v1"),
        paper_url: Some("https://www.biorxiv.org/content/10.1101/2025.09.19.677421v1"),
        license: License::Mit,
        license_url: None,
    },
};
