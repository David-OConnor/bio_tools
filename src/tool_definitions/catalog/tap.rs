use super::CatalogEntry;
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense};

/// A structural model is built before the five metrics are measured on
/// it, so this is not the sequence-only calculation it looks like.
pub const ENTRY: CatalogEntry = CatalogEntry {
    slug: "tap",
    name: "TAP",
    categories: &[
        ProcessCategory::PropertyPrediction,
        ProcessCategory::AntibodyDesign,
    ],
    launch_type: LaunchType::CondaBasedApp,
    license_type: LicenseType::NonCommercial,
    expense: ProcessExpense::Expensive,
    top_choice: true,
    summary: "Flag developability risks in an antibody against clinical-stage therapeutics.",
    description: "Builds a structural model of the variable domains and computes the five Therapeutic Antibody Profiler metrics on it: total CDR length, surface hydrophobicity, positive and negative charge patches, and the charge symmetry between the two chains. Each is reported against the range seen in antibodies that reached the clinic, so a value at or beyond the extremes is an amber or red flag rather than a hard failure.",
    availability: "Requires the licensed OPIG SAbPred distribution, installed and configured by the operator",
    license_details: "Academic use only. OPIG distributes TAP under its own licence, which has to be requested; commercial use needs a separate agreement, and nothing here is downloaded automatically as a result.",
    repo_url: None,
    home_url: Some("https://opig.stats.ox.ac.uk/webapps/sabdab-sabpred/sabpred/tap/"),
    docs_url: Some("https://www.pnas.org/doi/10.1073/pnas.1810576116"),
};
