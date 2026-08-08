use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

/// A whole saturation-mutagenesis scan in one forward pass, which is the
/// point of the method.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Alias {
        tool: Tool::ProteinMpnnDdg,
        slug: "proteinmpnn_ddg",
        name: None,
    },
    categories: &[ToolCategory::PropertyPrediction],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: true,
    spec: SpecData {
        summary: "Estimate changes in protein stability upon point mutation",
        description: "A modification of ProteinMPNN to use full sequence context. It introduces a decoding scheme \
        to improve computational efficiency and enable saturation mutagenesis studies at scale.",
        availability: "Installed by setup_system.sh with JAX CUDA 12 for Linux/WSL and an NVIDIA GPU",
        license_details: "MIT (Peptone), over MIT-licensed ProteinMPNN weights. Commercial use is unrestricted.",
        repo_url: Some("https://github.com/PeptoneLtd/proteinmpnn_ddg"),
        home_url: Some("https://peptone.io/"),
        docs_url: None,
        paper_url: Some("https://www.biorxiv.org/content/10.1101/2024.06.15.599145"),
        license: License::Mit,
        license_url: None,
    },
};
