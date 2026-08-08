use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::Mber),
    categories: &[
        ToolCategory::AntibodyDesign,
        ToolCategory::PeptideBinderDesign,
        ToolCategory::ProteinDesign,
    ],
    launch_type: LaunchType::CondaBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: false,
    spec: SpecData {
        summary: "A protein design framework for antibody binder design",
        description: "Controllable de novo antibody design with million-scale experimental screening. \
        mBER enables format specific binder design by leveraging structure templates and sequence \
        conditioning in backprop design through AlphaFold-Multimer.",
        availability: "Conda environment, an NVIDIA GPU, and the mber-open weights (AlphaFold 2, NanoBodyBuilder2, ESM2) are required",
        license_details: "MIT (Manifold Bio) over ColabDesign (Apache 2.0) and the AlphaFold 2 parameters (CC BY 4.0). Commercial use is permitted; confirm the upstream terms before relying on that.",
        repo_url: Some("https://github.com/manifoldbio/mber-open"),
        home_url: None,
        docs_url: Some("https://github.com/manifoldbio/mber-open/blob/main/protocols/README.md"),
        paper_url: Some("https://www.biorxiv.org/content/10.1101/2025.09.26.678877v1"),
        license: License::Mit,
        license_url: None,
    },
};
