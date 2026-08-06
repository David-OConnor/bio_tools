use super::CatalogEntry;
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense};

pub const ENTRY: CatalogEntry = CatalogEntry {
    slug: "biophi",
    name: "BioPhi",
    categories: &[
        ProcessCategory::AntibodyDesign,
        ProcessCategory::SequenceAnalysis,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    summary: "Humanize antibody sequences or estimate their humanness.",
    description: "Prepares paired antibody FASTA and invokes BioPhi Sapiens or OASis.",
    availability: "Installed by setup_system.sh into its own uv environment",
    license_details: "MIT (Merck). Commercial use is unrestricted.",
    repo_url: Some("https://github.com/Merck/BioPhi"),
    home_url: Some("https://biophi.dichlab.org/"),
    docs_url: None,
};
