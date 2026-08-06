use super::{CatalogEntry, Identity};
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense, tool_definitions::Tool};

/// A whole saturation-mutagenesis scan in one forward pass, which is the
/// point of the method.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Alias {
        tool: Tool::ProteinMpnnDdg,
        slug: "proteinmpnn_ddg",
        name: None,
    },
    categories: &[ProcessCategory::PropertyPrediction],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: true,
    summary: "Predict the stability effects of every point mutation in a protein chain.",
    description: "Runs ProteinMPNN-ddG with full sequence context and writes its saturation-mutagenesis scores as CSV.",
    availability: "Installed by setup_system.sh with JAX CUDA 12 for Linux/WSL and an NVIDIA GPU",
    license_details: "MIT (Peptone), over MIT-licensed ProteinMPNN weights. Commercial use is unrestricted.",
    repo_url: Some("https://github.com/PeptoneLtd/proteinmpnn_ddg"),
    home_url: Some("https://peptone.io/"),
    docs_url: None,
};
