use super::CatalogEntry;
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense};

/// A hosted model: the wait is a network round trip and a queue, not local
/// compute -- but it is billed against Boltz credits.
pub const ENTRY: CatalogEntry = CatalogEntry {
    slug: "boltz_adme",
    name: "Boltz ADME",
    categories: &[
        ProcessCategory::PropertyPrediction,
        ProcessCategory::Cheminformatics,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::Proprietary,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    summary: "Predict lipophilicity, permeability, and solubility for small molecules.",
    description: "Submits a batch of SMILES to Boltz's hosted adme-v1 model and returns the per-molecule Tier-1 ADME summary.",
    availability: "Requires the setup_system.sh client environment, network access, Boltz credits, and BOLTZ_API_KEY",
    license_details: "The only hosted tool in the registry, and the only one whose terms are a contract rather than a licence: the boltz-api client is open, but adme-v1 itself runs on Boltz's servers under their terms of service, billed against account credits. Molecules submitted here leave this host.",
    repo_url: None,
    home_url: Some("https://boltz.bio/"),
    docs_url: Some("https://api.boltz.bio/docs/guides/small-molecule-adme/"),
};
