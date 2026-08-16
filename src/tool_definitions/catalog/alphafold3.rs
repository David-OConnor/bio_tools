use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::AlphaFold3),
    categories: &[
        ToolCategory::StructurePrediction,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::NonCommercial,
    expense: ProcessExpense::Expensive,
    top_choice: true,
    spec: SpecData {
        summary: "Accurate structure prediction of biomolecular interactions; protein structure prediction",
        description: "The latest version of the most renowned protein structure prediction tool.",
        availability: "Model parameters, databases, and a GPU installation are required for execution",
        license_details: "Source code is Apache 2.0, but the model parameters are not: they are covered by the AlphaFold 3 Model Parameters Terms of Use, must be requested from and received directly from Google, may not be redistributed, and are for non-commercial use only. This is why the alphafold3 environment installs nothing and waits for an operator's own licensed checkout.",
        repo_url: Some("https://github.com/google-deepmind/alphafold3"),
        home_url: Some("https://deepmind.google/science/alphafold/"),
        docs_url: Some("https://github.com/google-deepmind/alphafold3/blob/main/docs/input.md"),
        paper_url: Some("https://doi.org/10.1038/s41586-024-07487-w"),
        license: License::Other,
        license_url: Some(
            "https://github.com/google-deepmind/alphafold3/blob/main/WEIGHTS_TERMS_OF_USE.md",
        ),
    },
};
