use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

/// Property prediction first: it scores mutations rather than producing
/// sequences, and is used to triage a design campaign's output rather than
/// to generate it.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::ThermoMpnn),
    categories: &[
        ToolCategory::PropertyPrediction,
        ToolCategory::ProteinDesign,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: true,
    spec: SpecData {
        summary: "A graph neural network (GNN) trained using transfer learning to predict changes in stability for protein point mutants",
        description: "A deep learning–based method for predicting thermostability changes quickly and accurately given \
        only an initial protein structure.",
        availability: "Installed by setup_system.sh into its own uv environment; weights ship with the checkout",
        license_details: "MIT (Kuhlman Lab), weights included, over MIT-licensed ProteinMPNN. Commercial use is unrestricted.",
        repo_url: Some("https://github.com/Kuhlman-Lab/ThermoMPNN"),
        home_url: None,
        docs_url: Some("https://www.pnas.org/doi/10.1073/pnas.2314853121"),
        paper_url: Some("https://www.pnas.org/doi/10.1073/pnas.2314853121"),
        license: License::Mit,
        license_url: None,
    },
};
