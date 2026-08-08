use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

/// One query against the germline databases: the process start and
/// database load dominate.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::IgBlast),
    categories: &[ToolCategory::SequenceAnalysis],
    launch_type: LaunchType::Executable,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    spec: SpecData {
        summary: "Identify germline V(D)J matches in an antibody sequence.",
        description: "Builds a FASTA query and invokes NCBI IgBLAST against the installed germline databases.",
        availability: "IgBLAST executable and germline databases installed by setup_system.sh",
        license_details: "Public domain: a US Government work from NCBI, with no copyright asserted and no restriction on commercial use. The IMGT-derived germline sets carry their own attribution terms.",
        repo_url: Some("https://github.com/ncbi/igblast"),
        home_url: Some("https://www.ncbi.nlm.nih.gov/igblast/"),
        docs_url: Some("https://ncbi.github.io/igblast/"),
        paper_url: Some("https://doi.org/10.1093/nar/gkt382"),
        license: License::PublicDomain,
        license_url: None,
    },
};
