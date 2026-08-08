use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::Protenix),
    categories: &[ToolCategory::StructurePrediction],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: true,
    spec: SpecData {
        summary: "Structure prediction for proteins, DNA/RNA, ligands, and ions.",
        description: "An advanced AI tool for fast, accurate, and user-friendly predictions of 3D structures \
        of all kinds of molecules, including proteins, ligands, ions, DNA, RNA, and covalent modifications.",
        availability: "Linux and a CUDA GPU are required; model weights download on first use.",
        license_details: "Apache 2.0 (ByteDance). Commercial use is permitted.",
        repo_url: Some("https://github.com/bytedance/Protenix"),
        home_url: Some("https://protenix-server.com/"),
        docs_url: None,
        paper_url: Some("https://www.biorxiv.org/content/10.1101/2025.01.08.631967"),
        license: License::ApacheV2,
        license_url: None,
    },
};
