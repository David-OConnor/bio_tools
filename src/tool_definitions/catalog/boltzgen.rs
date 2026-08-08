use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::BoltzGen),
    categories: &[
        ToolCategory::PeptideBinderDesign,
        ToolCategory::ProteinDesign,
        ToolCategory::AntibodyDesign,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: true,
    spec: SpecData {
        summary: "Designs proteins and peptides that bind to a wide range of biomolecular targets.",
        description: "It unifies design and structure prediction, resulting in a single model that also
        achieves state-of-the-art folding performance. 
BoltzGen was developed at MIT and experimentally validated in a large-scale distributed effort involving
multiple academic and industry labs.
Explicitly focuses our experimental validation on targets that are highly dissimilar to any proteins for
 which bound structures exist.",
        availability: "Installed by setup_system.sh into its own uv environment; approximately 6 GB of \
        model weights download separately",
        license_details: "MIT, covering the weights and training data as well as the inference code: unrestricted academic and commercial use.",
        repo_url: Some("https://github.com/HannesStark/boltzgen"),
        home_url: Some("https://boltz.bio/boltzgen"),
        docs_url: None,
        paper_url: Some("https://www.biorxiv.org/content/10.1101/2025.11.20.689494"),
        license: License::Mit,
        license_url: None,
    },
};
