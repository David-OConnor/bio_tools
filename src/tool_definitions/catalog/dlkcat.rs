use super::CatalogEntry;
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense};

pub const ENTRY: CatalogEntry = CatalogEntry {
    slug: "dlkcat",
    name: "DLKcat",
    categories: &[
        ProcessCategory::PropertyPrediction,
        ProcessCategory::Cheminformatics,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    summary: "Predict an enzyme turnover number from its sequence and a substrate structure.",
    description: "Runs the deep-learning half of the DLKcat toolbox, which pairs a graph neural network over the substrate with a convolutional network over the enzyme sequence to predict kcat. It is trained on wild-type and mutant enzymes across many organisms, and is meant for parameterising models rather than for ranking closely related variants.",
    availability: "Installed by setup_system.sh into its own uv environment; the trained model ships with the checkout",
    license_details: "MIT (SysBioChalmers), weights included. Commercial use is unrestricted.",
    repo_url: Some("https://github.com/SysBioChalmers/DLKcat"),
    home_url: None,
    docs_url: Some("https://www.nature.com/articles/s41929-022-00798-z"),
};
