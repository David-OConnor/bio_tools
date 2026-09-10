use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::{
        Tool,
        catalog::{CatalogEntry, Identity},
    },
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::Boltz2),
    categories: &[
        ToolCategory::StructurePrediction,
        ToolCategory::PropertyPrediction,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: true,
    spec: SpecData {
        summary: "All-atom biomolecular structure and binding-affinity prediction.",
        description: "Models complex structures and binding affinities, a critical component \
        towards accurate molecular design. Boltz-2 is the first deep learning model to approach the accuracy of physics-based \
        free-energy perturbation (FEP) methods, while running 1000x faster — making accurate in silico screening practical for \
        early-stage drug discovery.",
        availability: "Installed by setup_system.sh into its own uv environment; model weights download on first execution",
        license_details: "MIT, covering the model weights as well as the code: unrestricted academic and commercial use.",
        repo_url: Some("https://github.com/jwohlwend/boltz"),
        home_url: Some("https://boltz.bio/"),
        docs_url: Some("https://github.com/jwohlwend/boltz/blob/main/docs/prediction.md"),
        input_params_url: Some(
            "https://github.com/jwohlwend/boltz/blob/main/docs/prediction.md#input-format",
        ),
        examples_url: Some("https://github.com/jwohlwend/boltz/tree/main/examples"),
        paper_url: Some("https://www.biorxiv.org/content/10.1101/2025.06.14.659707"),
        license: License::Mit,
        license_url: None,
        tested: true,
    },
};
