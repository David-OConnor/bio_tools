use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::RfDiffusion),
    categories: &[
        ToolCategory::ProteinDesign,
        ToolCategory::PeptideBinderDesign,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: true,
    spec: SpecData {
        summary: "Generate protein backbones unconditionally, around a motif, or against a target.",
        description: "Runs the official RFdiffusion inference script. Custom Contigs exposes the raw contig map directly; the other tasks build one for you from a target/binder/motif description.",
        availability: "Installed by setup_system.sh, which fetches the public model weights; Linux/WSL and an NVIDIA GPU are required",
        license_details: "A BSD-style licence from the University of Washington, which upstream states is free for both non-profit and for-profit use. The public checkpoints setup_system.sh fetches are under the same terms.",
        repo_url: Some("https://github.com/RosettaCommons/RFdiffusion"),
        home_url: None,
        docs_url: None,
        paper_url: Some("https://doi.org/10.1038/s41586-023-06415-8"),
        license: License::Other,
        license_url: Some("https://github.com/RosettaCommons/RFdiffusion/blob/main/LICENSE"),
    },
};
