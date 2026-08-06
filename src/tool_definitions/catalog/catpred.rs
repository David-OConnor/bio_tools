use super::{CatalogEntry, Identity};
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense, tool_definitions::Tool};

/// A protein language model embedding per query, unlike DLKcat's much
/// smaller sequence CNN.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::CatPred),
    categories: &[
        ProcessCategory::PropertyPrediction,
        ProcessCategory::Cheminformatics,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: false,
    summary: "Predict kcat, Km, or Ki for an enzyme and substrate, with an uncertainty estimate.",
    description: "Runs CatPred, which combines a pretrained protein language model with a molecular representation of the substrate and predicts a distribution rather than a point value, so each prediction carries a variance that tracks how far the query sits from the training data.",
    availability: "Installed by setup_system.sh into its own uv environment; the checkpoint archive downloads automatically, and setup_system.sh symlinks one ensemble per parameter (seed0, picked arbitrarily among the ten this research-reproduction bundle ships) into a consistent kcat/km/ki layout under CATPRED_CHECKPOINT_DIR. The ki checkpoint needs pretrained EGNN features this adapter does not supply, so that parameter is not fully wired up",
    license_details: "MIT (Maranas group), weights included. Commercial use is unrestricted; confirm the repository's own licence before relying on that.",
    repo_url: Some("https://github.com/maranasgroup/CatPred"),
    home_url: Some("https://www.catpred.com/"),
    docs_url: Some("https://www.nature.com/articles/s41467-025-57215-9"),
};
