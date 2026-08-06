use super::CatalogEntry;
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense};

pub const ENTRY: CatalogEntry = CatalogEntry {
    slug: "esmfold2",
    name: "ESMFold 2",
    categories: &[
        ProcessCategory::StructurePrediction,
        ProcessCategory::ProteinDesign,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: false,
    summary: "Fold a protein from its sequence alone, with no MSA or template search.",
    description: "Writes a FASTA record and invokes the esm-fold CLI from fair-esm. Predictions are single-sequence, so no alignment or template database is consulted.",
    availability: "Installed by setup_system.sh into its own uv environment (needs nvcc to build OpenFold); ESM-2 weights download on first execution",
    license_details: "fair-esm and the ESM-2 weights are MIT; the OpenFold dependency this environment builds is Apache 2.0. Both allow commercial use.",
    repo_url: Some("https://github.com/facebookresearch/esm"),
    home_url: Some("https://esmatlas.com/"),
    docs_url: Some("https://github.com/facebookresearch/esm#esmfold"),
};
