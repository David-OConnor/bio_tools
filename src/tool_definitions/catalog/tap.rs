use super::{CatalogEntry, Identity};
use crate::{LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory};

/// A structural model is built before the five metrics are measured on
/// it, so this is not the sequence-only calculation it looks like.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Uninstalled {
        slug: "tap",
        name: "TAP",
    },
    categories: &[
        ToolCategory::PropertyPrediction,
        ToolCategory::AntibodyDesign,
    ],
    launch_type: LaunchType::CondaBasedApp,
    license_type: LicenseCategory::NonCommercial,
    expense: ProcessExpense::Expensive,
    top_choice: true,
    spec: SpecData {
        summary: "A therapeutic antibody profiler",
        description: "Compares your antibody variable domain sequence against five developability guidelines \
        derived from clinical-stage therapeutic values.",
        availability: "Requires the licensed OPIG SAbPred distribution, installed and configured by the operator",
        license_details: "Academic use only. OPIG distributes TAP under its own licence, which has to be requested; commercial use needs a separate agreement, and nothing here is downloaded automatically as a result.",
        repo_url: None,
        home_url: Some("https://opig.stats.ox.ac.uk/webapps/sabdab-sabpred/sabpred/tap"),
        docs_url: Some("https://www.pnas.org/doi/10.1073/pnas.1810576116"),
        paper_url: Some("https://www.pnas.org/doi/10.1073/pnas.1810576116"),
        license: License::Other,
        license_url: None,
    },
};
