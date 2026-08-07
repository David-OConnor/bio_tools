use super::{CatalogEntry, Identity};
use crate::{LaunchType, LicenseCategory, ProcessExpense, ToolCategory, tool_definitions::Tool};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::DeepImmuno),
    categories: &[
        ToolCategory::PropertyPrediction,
        ToolCategory::SequenceAnalysis,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    summary: "Score how likely a peptide-MHC class I pair is to provoke a CD8 T-cell response.",
    description: "Runs the DeepImmuno convolutional model over 9- or 10-mer epitopes paired with an HLA class I allele, returning an immunogenicity score per pair. Binding prediction is a separate question: this scores whether a presented peptide is recognised, not whether it is presented.",
    availability: "Installed by setup_system.sh into its own uv environment; the trained model ships with the checkout",
    license_details: "MIT (Cincinnati Children's Hospital Medical Center), weights included. Confirm the repository's own licence before commercial use.",
    repo_url: Some("https://github.com/frankligy/DeepImmuno"),
    home_url: Some("https://deepimmuno.research.cchmc.org/"),
    docs_url: None,
};
