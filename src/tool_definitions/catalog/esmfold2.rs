use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::EsmFold2),
    categories: &[
        ToolCategory::StructurePrediction,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: false,
    spec: SpecData {
        summary: "Protein structure prediction; fast.",
        description: "Similar use cases to AlphaFold3, Boltz-2, and OpenDDE. Faster, but can only fold \
        single-chain proteins, and doesn't support other molecule types.",
        availability: "Installed by setup_system.sh into its own uv environment (needs nvcc to build OpenFold); ESM-2 weights download on first execution",
        license_details: "fair-esm and the ESM-2 weights are MIT; the OpenFold dependency this environment builds is Apache 2.0. Both allow commercial use.",
        repo_url: Some("https://github.com/facebookresearch/esm"),
        home_url: Some("https://esmatlas.com/"),
        docs_url: Some("https://github.com/facebookresearch/esm#esmfold"),
        paper_url: Some("https://www.science.org/doi/10.1126/science.ade2574"),
        license: License::Mit,
        license_url: None,
    },
};
