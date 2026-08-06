use super::{CatalogEntry, Identity};
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense, tool_definitions::Tool};

/// One query against the germline databases: the process start and
/// database load dominate.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::IgBlast),
    categories: &[ProcessCategory::SequenceAnalysis],
    launch_type: LaunchType::Executable,
    license_type: LicenseType::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    summary: "Identify germline V(D)J matches in an antibody sequence.",
    description: "Builds a FASTA query and invokes NCBI IgBLAST against the installed germline databases.",
    availability: "IgBLAST executable and germline databases installed by setup_system.sh",
    license_details: "Public domain: a US Government work from NCBI, with no copyright asserted and no restriction on commercial use. The IMGT-derived germline sets carry their own attribution terms.",
    repo_url: Some("https://github.com/ncbi/igblast"),
    home_url: Some("https://www.ncbi.nlm.nih.gov/igblast/"),
    docs_url: Some("https://ncbi.github.io/igblast/"),
};
