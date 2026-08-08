use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::BioPhi),
    categories: &[ToolCategory::AntibodyDesign, ToolCategory::SequenceAnalysis],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    spec: SpecData {
        summary: "Humanize antibody sequences or estimate their humanness.",
        description: "Prepares paired antibody FASTA and invokes BioPhi Sapiens or OASis.",
        availability: "Installed by setup_system.sh into its own uv environment",
        license_details: "MIT (Merck). Commercial use is unrestricted.",
        repo_url: Some("https://github.com/Merck/BioPhi"),
        home_url: Some("https://biophi.dichlab.org/"),
        docs_url: None,
        paper_url: Some("https://doi.org/10.1080/19420862.2021.2020203"),
        license: License::Mit,
        license_url: None,
    },
};
