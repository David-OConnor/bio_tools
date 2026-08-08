use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

/// A hosted model: the wait is a network round trip and a queue, not local
/// compute -- but it is billed against Boltz credits.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Alias {
        tool: Tool::BoltzAdme,
        slug: "boltz_adme",
        name: None,
    },
    categories: &[
        ToolCategory::PropertyPrediction,
        ToolCategory::Cheminformatics,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Proprietary,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    spec: SpecData {
        summary: "Predict Tier-1 ADME summary properties (lipophilicity, permeability, and solubility) for \
        a batch of small molecules by SMILES.",
        description: "ADME prediction scores a batch of small molecules for Tier-1 ADME summary properties \
        (lipophilicity, permeability, and solubility) directly from SMILES. You submit a list of molecules and \
        get back one result object with a per-molecule summary, returned in the same order you submitted them. \
        It runs to completion and cannot be paused or stopped.",
        availability: "Requires the setup_system.sh client environment, network access, Boltz credits, and BOLTZ_API_KEY",
        license_details: "The only hosted tool in the registry, and the only one whose terms are a contract rather than a licence: the boltz-api client is open, but adme-v1 itself runs on Boltz's servers under their terms of service, billed against account credits. Molecules submitted here leave this host.",
        repo_url: None,
        home_url: Some("https://boltz.bio/"),
        docs_url: Some("https://api.boltz.bio/docs/guides/small-molecule-adme/"),
        paper_url: None,
        license: License::Other,
        license_url: None,
    },
};
