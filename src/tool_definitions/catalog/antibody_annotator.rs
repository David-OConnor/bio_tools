use super::{CatalogEntry, Identity};
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense, tool_definitions::Tool};

/// Sequence analysis first: it numbers and annotates what was submitted
/// rather than predicting anything about it, and the liability flags
/// follow from the regions the numbering establishes.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Alias {
        tool: Tool::Anarcii,
        slug: "antibody_annotator",
        name: Some("Antibody Annotator"),
    },
    categories: &[
        ProcessCategory::SequenceAnalysis,
        ProcessCategory::AntibodyDesign,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    summary: "Number an antibody or TCR sequence and report its regions and liabilities.",
    description: "Numbers each variable domain with the chosen scheme, reports the framework and CDR boundaries that follow from it, and flags the sequence motifs associated with glycosylation, deamidation, isomerisation, and oxidation in each region.",
    availability: "Installed by setup_system.sh into its own uv environment; the numbering models ship with the package",
    license_details: "BSD 3-Clause (Oxford Protein Informatics Group). Commercial use is unrestricted, and the liability scan below is this project's own code.",
    repo_url: Some("https://github.com/oxpig/ANARCII"),
    home_url: Some("https://opig.stats.ox.ac.uk/webapps/sabdab-sabpred/sabpred/anarci/"),
    docs_url: None,
};
