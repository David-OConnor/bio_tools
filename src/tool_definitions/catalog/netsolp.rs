use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

/// A protein language model embedding per sequence, and ESM-1b is the
/// large one of the three offered.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::NetSolP),
    categories: &[
        ToolCategory::PropertyPrediction,
        ToolCategory::SequencePrediction,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::NonCommercial,
    expense: ProcessExpense::Expensive,
    top_choice: false,
    spec: SpecData {
        summary: "Predict whether a protein will be soluble and usable when expressed in E. coli.",
        description: "Runs NetSolP over a set of FASTA sequences, scoring each for solubility and for usability, which combines solubility with expressibility. The predictions come from a protein language model and need no structure.",
        availability: "Installed by setup_system.sh into its own uv environment; the model weights download separately from DTU",
        license_details: "Academic use only: DTU Health Tech licenses NetSolP and its weights for non-commercial research, and commercial use needs a separate agreement.",
        repo_url: Some("https://github.com/tvinet/NetSolP-1.0"),
        home_url: Some("https://services.healthtech.dtu.dk/services/NetSolP-1.0/"),
        docs_url: Some("https://academic.oup.com/bioinformatics/article/38/4/941/6444984"),
        paper_url: Some("https://academic.oup.com/bioinformatics/article/38/4/941/6444984"),
        license: License::Other,
        license_url: None,
    },
};
