use super::CatalogEntry;
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense};

pub const ENTRY: CatalogEntry = CatalogEntry {
    slug: "protenix",
    name: "Protenix-v2",
    categories: &[ProcessCategory::StructurePrediction],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::Permissive,
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
