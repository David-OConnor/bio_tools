use super::{CatalogEntry, Identity};
use crate::{LaunchType, LicenseCategory, ProcessExpense, ToolCategory, tool_definitions::Tool};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::AntiFold),
    categories: &[
        ToolCategory::AntibodyDesign,
        ToolCategory::SequencePrediction,
    ],
    launch_type: LaunchType::CondaBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    summary: "Score or sample antibody sequences from an IMGT-numbered structure.",
    description: "Invokes AntiFold on a supplied antibody or antibody-antigen PDB structure.",
    availability: "Separate AntiFold, PyTorch, and model installation required",
    license_details: "BSD 3-Clause (Oxford Protein Informatics Group).",
    repo_url: Some("https://github.com/oxpig/AntiFold"),
    home_url: Some("https://opig.stats.ox.ac.uk/webapps/antifold/"),
    docs_url: None,
};
