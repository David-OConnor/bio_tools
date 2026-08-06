use super::CatalogEntry;
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense};

/// Property prediction is the third category rather than one of the two it
/// shares with the co-folding models above: the adapter asks for ligand
/// affinity by default, which none of the others predict.
pub const ENTRY: CatalogEntry = CatalogEntry {
    slug: "boltz2",
    name: "Boltz-2",
    categories: &[
        ProcessCategory::StructurePrediction,
        ProcessCategory::ProteinDesign,
        ProcessCategory::PropertyPrediction,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: true,
    summary: "Predict a biomolecular complex and optional ligand affinity.",
    description: "Builds the preferred Boltz YAML input and invokes the official boltz CLI. Runs single-sequence by default: no MSA server is contacted unless explicitly enabled.",
    availability: "Installed by setup_system.sh into its own uv environment; model weights download on first execution",
    license_details: "MIT, covering the model weights as well as the code: unrestricted academic and commercial use.",
    repo_url: Some("https://github.com/jwohlwend/boltz"),
    home_url: Some("https://boltz.bio/"),
    docs_url: Some("https://github.com/jwohlwend/boltz/blob/main/docs/prediction.md"),
};
