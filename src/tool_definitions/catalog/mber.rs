use super::{CatalogEntry, Identity};
use crate::{LaunchType, LicenseCategory, ProcessExpense, ToolCategory, tool_definitions::Tool};

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
    summary: "Design VHH (nanobody) binders against a target by backprop through AlphaFold-Multimer.",
    description: "Runs mBER (Manifold Binder Engineering and Refinement) through its mber-vhh command-line tool, which designs VHH binders with the ColabDesign backpropagation loop against a target structure, optionally steered onto named hotspots, and keeps every trajectory that clears its iPTM and pLDDT filters. scFv design is not exposed by this CLI in the open-source release.",
    availability: "Conda environment, an NVIDIA GPU, and the mber-open weights (AlphaFold 2, NanoBodyBuilder2, ESM2) are required",
    license_details: "MIT (Manifold Bio) over ColabDesign (Apache 2.0) and the AlphaFold 2 parameters (CC BY 4.0). Commercial use is permitted; confirm the upstream terms before relying on that.",
    repo_url: Some("https://github.com/manifoldbio/mber-open"),
    home_url: None,
    docs_url: Some("https://www.biorxiv.org/content/10.1101/2025.09.26.678877v1"),
};
