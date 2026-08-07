use super::{CatalogEntry, Identity};
use crate::{LaunchType, LicenseCategory, ProcessExpense, ToolCategory, tool_definitions::Tool};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Alias {
        tool: Tool::TlImmuno2,
        slug: "tlimmuno",
        name: None,
    },
    categories: &[
        ToolCategory::PropertyPrediction,
        ToolCategory::SequenceAnalysis,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    summary: "Score how likely a peptide-MHC class II pair is to provoke a CD4 T-cell response.",
    description: "Runs TLimmuno2, an LSTM model transferred from class II binding affinity data onto immunogenicity, over peptides paired with an MHC class II allele. Useful for anti-drug-response risk on a biologic as well as for vaccine epitope selection.",
    availability: "Installed by setup_system.sh into its own uv environment; the trained model ships with the checkout",
    license_details: "Academic release from the Liu lab, weights included. Confirm the repository's own licence before commercial use.",
    repo_url: Some("https://github.com/XSLiuLab/TLimmuno2"),
    home_url: Some("https://xsliulab.github.io/TLimmuno2/"),
    docs_url: Some("https://academic.oup.com/bib/article/24/3/bbad116/7084794"),
};
