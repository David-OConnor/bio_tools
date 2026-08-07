use super::{CatalogEntry, Identity};
use crate::{LaunchType, LicenseCategory, ProcessExpense, ToolCategory, tool_definitions::Tool};

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
    summary: "Predict an antibody, nanobody, or TCR structure in seconds.",
    description: "Runs ABodyBuilder2, NanoBodyBuilder2, or TCRBuilder2 on the supplied variable-domain sequences. These are single-sequence models with no MSA or template search, so a structure comes back in seconds rather than the minutes a general folding model needs.",
    availability: "Installed by setup_system.sh into its own uv environment; weights download on first execution",
    license_details: "BSD 3-Clause (Oxford Protein Informatics Group), weights included. Commercial use is unrestricted. Refinement uses OpenMM, which is MIT/LGPL.",
    repo_url: Some("https://github.com/oxpig/ImmuneBuilder"),
    home_url: Some("https://opig.stats.ox.ac.uk/webapps/sabdab-sabpred/sabpred/abodybuilder2/"),
    docs_url: None,
};
