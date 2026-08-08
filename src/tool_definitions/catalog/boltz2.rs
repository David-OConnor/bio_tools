use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

/// Property prediction is the third category rather than one of the two it
/// shares with the co-folding models above: the adapter asks for ligand
/// affinity by default, which none of the others predict.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::Boltz2),
    categories: &[
        ToolCategory::StructurePrediction,
        ToolCategory::ProteinDesign,
        ToolCategory::PropertyPrediction,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: true,
    spec: SpecData {
        summary: "Biomolecular interaction prediction, and protein structure prediction.",
        description: "Models complex structures and binding affinities, a critical component \
        towards accurate molecular design. Boltz-2 is the first deep learning model to approach the accuracy of physics-based \
        free-energy perturbation (FEP) methods, while running 1000x faster — making accurate in silico screening practical for \
        early-stage drug discovery.",
        availability: "Installed by setup_system.sh into its own uv environment; model weights download on first execution",
        license_details: "MIT, covering the model weights as well as the code: unrestricted academic and commercial use.",
        repo_url: Some("https://github.com/jwohlwend/boltz"),
        home_url: Some("https://boltz.bio/"),
        docs_url: Some("https://github.com/jwohlwend/boltz/blob/main/docs/prediction.md"),
        paper_url: Some("https://www.biorxiv.org/content/10.1101/2025.06.14.659707"),
        license: License::Mit,
        license_url: None,
    },
};
