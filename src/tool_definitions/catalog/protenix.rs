use super::{CatalogEntry, Identity};
use crate::{LaunchType, LicenseCategory, ProcessExpense, ToolCategory, tool_definitions::Tool};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::Protenix),
    categories: &[ToolCategory::StructurePrediction],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: true,
    summary: "Predict protein or protein-ligand complex structures with Protenix.",
    description: "Builds Protenix's AlphaFold3-style JSON input and runs the official `protenix pred` CLI, with MSA and PDB template search off by default.",
    availability: "Linux and a CUDA GPU are required; model weights download on first use.",
    license_details: "Apache 2.0 (ByteDance). Commercial use is permitted.",
    repo_url: Some("https://github.com/bytedance/Protenix"),
    home_url: Some("https://protenix-server.com/"),
    docs_url: None,
};
