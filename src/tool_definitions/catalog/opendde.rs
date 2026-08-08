use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::OpenDde),
    categories: &[
        ToolCategory::StructurePrediction,
        ToolCategory::ProteinDesign,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: true,
    spec: SpecData {
        summary: "Structure prediction for proteins, DNA/RNA, ligands, and ions. Supports co-folding.",
        description: "OpenDDE turns co-folding into a scalable engine for structure prediction, design, \
         and optimization — modeling proteins, nucleic acids, and small molecules in one all-atom system.",
        availability: "Installed by setup_system.sh into its own uv environment; model assets download separately",
        license_details: "Apache 2.0 (Aureka Research). Commercial use is permitted, with the licence's attribution and notice conditions.",
        repo_url: Some("https://github.com/aurekaresearch/OpenDDE"),
        home_url: Some("https://aurekaresearch.github.io/OpenDDE-Website/"),
        docs_url: Some("https://github.com/aurekaresearch/OpenDDE/blob/main/docs/tutorial.md"),
        paper_url: Some("https://arxiv.org/abs/2607.03787"),
        license: License::ApacheV2,
        license_url: None,
    },
};
