use super::CatalogEntry;
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense};

pub const ENTRY: CatalogEntry = CatalogEntry {
    slug: "alphafold3",
    name: "AlphaFold 3",
    categories: &[
        ProcessCategory::StructurePrediction,
        ProcessCategory::ProteinDesign,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::NonCommercial,
    expense: ProcessExpense::Expensive,
    top_choice: true,
    summary: "Prepare protein or protein-ligand AlphaFold 3 predictions.",
    description: "Creates the official AlphaFold 3 JSON format and can invoke a separately configured installation.",
    availability: "Model parameters, databases, and a GPU installation are required for execution",
    license_details: "Source code is Apache 2.0, but the model parameters are not: they are covered by the AlphaFold 3 Model Parameters Terms of Use, must be requested from and received directly from Google, may not be redistributed, and are for non-commercial use only. This is why the alphafold3 environment installs nothing and waits for an operator's own licensed checkout.",
    repo_url: Some("https://github.com/google-deepmind/alphafold3"),
    home_url: Some("https://deepmind.google/science/alphafold/"),
    docs_url: Some("https://github.com/google-deepmind/alphafold3/blob/main/docs/input.md"),
};
