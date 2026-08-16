use super::{CatalogEntry, Identity};
use crate::{LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory};

/// RetroBioCat 2 is the maintained downloadable Python synthesis-planning package. The separate
/// RetroBioCat website exposes the curated database interactively but does not publish a stable
/// public database API.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Uninstalled {
        slug: "retrobiocat",
        name: "RetroBioCat 2",
    },
    categories: &[ToolCategory::Cheminformatics, ToolCategory::ProteinDesign],
    launch_type: LaunchType::PythonLib,
    license_type: LicenseCategory::NonCommercial,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    spec: SpecData {
        summary: "Plan biocatalytic routes and connect reaction rules to candidate enzyme classes.",
        description: "RetroBioCat 2 is a modular computer-aided synthesis-planning library for biocatalysis. It applies biocatalytic reaction rules, searches routes, and supports hybrid planning with chemical transformations; the companion RetroBioCat database provides curated synthetic biotransformations, enzyme sequences, substrate scope, and reaction conditions through its website.",
        availability: "Install with pip from the upstream Git repository, preferably in a dedicated Conda environment because its stack includes HDF5; no unattended bio_tools recipe is provided",
        license_details: "CC BY-NC 4.0. Attribution is required and commercial use is not permitted without separate permission.",
        repo_url: Some("https://github.com/willfinnigan/RetroBioCat-2"),
        home_url: Some("https://retrobiocat.com/"),
        docs_url: Some("https://retrobiocat-2.readthedocs.io/en/latest/"),
        paper_url: Some("https://doi.org/10.1021/acscatal.3c01418"),
        license: License::Other,
        license_url: Some("https://github.com/willfinnigan/RetroBioCat-2/blob/main/LICENSE.md"),
    },
};
