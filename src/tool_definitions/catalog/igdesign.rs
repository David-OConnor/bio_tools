use super::CatalogEntry;
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense};

pub const ENTRY: CatalogEntry = CatalogEntry {
    slug: "igdesign",
    name: "IgDesign",
    categories: &[
        ProcessCategory::AntibodyDesign,
        ProcessCategory::SequencePrediction,
        ProcessCategory::ProteinDesign,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::NonCommercial,
    expense: ProcessExpense::Expensive,
    top_choice: false,
    summary: "Design antibody CDRs against a target antigen by inverse folding.",
    description: "Runs IgDesign, which conditions on an antigen-antibody complex structure plus the antibody framework sequence and samples new sequences for the CDRs it is given explicit position ranges for. IgDesign's own configuration format takes those ranges directly rather than a numbering scheme, so this does too.",
    availability: "Installed by setup_system.sh into its own uv environment; needs Linux, an NVIDIA GPU, and the published checkpoints",
    license_details: "Released by Absci for research use, weights included. Confirm the repository's own licence before any commercial application.",
    repo_url: Some("https://github.com/AbSciBio/igdesign"),
    home_url: Some("https://www.absci.com/antibody-inverse-folding/"),
    docs_url: Some("https://www.biorxiv.org/content/10.1101/2023.12.08.570889v2"),
};
