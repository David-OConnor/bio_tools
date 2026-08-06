use super::CatalogEntry;
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense};

/// Both: the binders are mini-proteins of 65-120 residues rather than
/// peptides, but PeptideBinderDesign is the registry's general "binder
/// design" category, which is what a campaign against a named target is.
pub const ENTRY: CatalogEntry = CatalogEntry {
    slug: "bindcraft",
    name: "BindCraft",
    categories: &[
        ProcessCategory::PeptideBinderDesign,
        ProcessCategory::ProteinDesign,
    ],
    launch_type: LaunchType::CondaBasedApp,
    license_type: LicenseType::NonCommercial,
    expense: ProcessExpense::Expensive,
    top_choice: true,
    summary: "Design de novo protein or peptide binders against a target structure.",
    description: "Builds a BindCraft target/advanced/filter settings trio and runs the official pipeline end to end (hallucination, ProteinMPNN redesign, AlphaFold2 validation, and filtering).",
    availability: "Linux, Conda/Mamba, AlphaFold weights, and a large GPU are required",
    license_details: "BindCraft itself is MIT, but it cannot run without PyRosetta, which is free only for non-commercial and academic use and needs a paid licence from the University of Washington otherwise. The AlphaFold2 weights it uses are CC BY 4.0.",
    repo_url: Some("https://github.com/martinpacesa/BindCraft"),
    home_url: None,
    docs_url: Some("https://github.com/martinpacesa/BindCraft/wiki"),
};
