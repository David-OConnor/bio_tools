use super::CatalogEntry;
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense};

pub const ENTRY: CatalogEntry = CatalogEntry {
    slug: "deepstabp",
    name: "DeepSTABp",
    categories: &[ProcessCategory::PropertyPrediction],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: false,
    summary: "Predict the melting temperature of a protein from its sequence.",
    description: "Runs DeepSTABp, which embeds each sequence with a protein language model and predicts the melting temperature it would show in a thermal proteome profiling experiment, conditioned on the growth temperature and on whether the measurement is on cells or lysate.",
    availability: "Installed by setup_system.sh into its own uv environment; the language model downloads on first execution",
    license_details: "MIT (CSBiology), weights included. Commercial use is unrestricted; confirm the repository's own licence before relying on that.",
    repo_url: Some("https://github.com/CSBiology/deepStabP"),
    home_url: Some("https://csb-deepstabp.bio.rptu.de/"),
    docs_url: None,
};
