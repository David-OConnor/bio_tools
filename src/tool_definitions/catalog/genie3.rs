use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

/// Confirmed: Genie 3's headline capability is target-conditioned binder
/// backbone generation, and the adapter requires a target structure,
/// selection, and hotspots.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::Genie3),
    categories: &[
        ToolCategory::PeptideBinderDesign,
        ToolCategory::ProteinDesign,
    ],
    launch_type: LaunchType::CondaBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: false,
    spec: SpecData {
        summary: "Fast protein design through all-atom SE(3)-equivariance",
        description: "A fast, all-atom SE(3)-equivariant diffusion model for protein design. \
        It achieves state-of-the-art performance on unconditional generation, motif scaffolding, and binder \
        design while retaining the computational efficiency of equivariant architectures.",
        availability: "Installed by setup_system.sh into its upstream Conda environment; Linux and a CUDA GPU are required",
        license_details: "Apache 2.0 (AlQuraishi Laboratory). Commercial use is permitted.",
        repo_url: Some("https://github.com/aqlaboratory/genie3"),
        home_url: Some("https://www.aqlab.io/"),
        docs_url: None,
        paper_url: Some("https://www.biorxiv.org/content/10.1101/2026.05.01.722168v1"),
        license: License::ApacheV2,
        license_url: None,
    },
};
