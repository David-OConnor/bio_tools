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
        summary: "Predict the stability change of point mutations in a protein structure.",
        description: "Runs ThermoMPNN, a ProteinMPNN-derived graph network trained by transfer learning to predict ddG for point mutants. custom_inference.py always scores every substitution at every position of a chain (a saturation scan); a named list of mutations is served by filtering that scan down to the requested substitutions rather than by asking the tool for them specifically, which its own CLI has no option for.",
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
