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
        summary: "A machine learning tool for generating protein backbone structures from minimal input.",
        description: "The potential uses of RFdiffusion range from protein monomer backbone design to scaffolding \
        enzyme active sites. The success of the tool across these disparate use cases hinges upon its use of denoising \
        diffusion probabilistic models (DDPMs), often referred to as diffusion models. Diffusion models are used in \
        image and music generation tools due to their ability to generate highly diverse outputs. In the case \
        of RFdiffusion, this means that the tool generates a variety of possible protein backbone structures \
        that still resemble the training data.",
        availability: "Installed by setup_system.sh, which fetches the public model weights; Linux/WSL and an NVIDIA GPU are required",
        license_details: "A BSD-style licence from the University of Washington, which upstream states is free for both non-profit and for-profit use. The public checkpoints setup_system.sh fetches are under the same terms.",
        repo_url: Some("https://github.com/RosettaCommons/RFdiffusion"),
        home_url: Some("https://sites.google.com/omsf.io/rfdiffusion"),
        docs_url: Some("https://sites.google.com/omsf.io/rfdiffusion/reference-docs/configuration-options"),
        paper_url: Some("https://doi.org/10.1038/s41586-023-06415-8"),
        license: License::Other,
        license_url: Some("https://github.com/RosettaCommons/RFdiffusion/blob/main/LICENSE"),
    },
};
