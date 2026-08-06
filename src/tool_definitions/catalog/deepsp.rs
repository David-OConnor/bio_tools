use super::CatalogEntry;
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense};

/// The point of the model: it replaces a molecular dynamics run with one
/// forward pass over the sequence.
pub const ENTRY: CatalogEntry = CatalogEntry {
    slug: "deepsp",
    name: "DeepSP",
    categories: &[
        ProcessCategory::PropertyPrediction,
        ProcessCategory::AntibodyDesign,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    summary: "Predict 30 spatial developability descriptors for an antibody from sequence alone.",
    description: "Runs DeepSP, a surrogate CNN that reproduces the spatial aggregation propensity and spatial charge map scores normally obtained from a molecular dynamics run, in each region of the variable domains, from the heavy and light chain sequences alone.",
    availability: "Installed by setup_system.sh into its own uv environment; the trained model ships with the checkout",
    license_details: "MIT (Lai Lab), weights included. Commercial use is unrestricted; confirm the repository's own licence before relying on that.",
    repo_url: Some("https://github.com/Lailabcode/DeepSP"),
    home_url: None,
    docs_url: Some("https://www.csbj.org/article/S2001-0370(24)00173-9/fulltext"),
};
