use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

/// Seconds, not minutes: three small single-sequence networks with no MSA
/// or template search, and the OpenMM refinement that follows each one is
/// what dominates the wait.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::ImmuneBuilder),
    categories: &[
        ToolCategory::StructurePrediction,
        ToolCategory::AntibodyDesign,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: true,
    spec: SpecData {
        summary: "Deep-Learning models for predicting the structures of immune proteins.",
        description: "A set of deep learning models trained to accurately predict the structure of antibodies \
        (ABodyBuilder2), nanobodies (NanoBodyBuilder2) and T-Cell receptors (TCRBuilder2). By predicting an ensemble \
        of structures, ImmuneBuilder also gives an error estimate for every residue in its final prediction.",
        availability: "Installed by setup_system.sh into its own uv environment; weights download on first execution",
        license_details: "BSD 3-Clause (Oxford Protein Informatics Group), weights included. Commercial use is unrestricted. Refinement uses OpenMM, which is MIT/LGPL.",
        repo_url: Some("https://github.com/oxpig/ImmuneBuilder"),
        home_url: Some("https://opig.stats.ox.ac.uk/webapps/sabdab-sabpred/sabpred/abodybuilder2/"),
        docs_url: None,
        paper_url: Some("https://doi.org/10.1038/s42003-023-04927-7"),
        license: License::Bsd3Clause,
        license_url: None,
    },
};
