use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::RfAntibody),
    categories: &[ToolCategory::AntibodyDesign, ToolCategory::ProteinDesign],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: false,
    spec: SpecData {
        summary: "Design antibody or nanobody binders against a target structure.",
        description: "Runs the whole RFantibody pipeline against a target and an HLT framework: antibody-finetuned RFdiffusion \
         docks a backbone and rebuilds the chosen CDR loops, ProteinMPNN designs their sequences, and antibody-finetuned RF2 \
         predicts the complex for filtering. The Antibody task designs a paired heavy/light framework; Nanobody designs a VHH.",
        availability: "Linux, an NVIDIA GPU, RFantibody weights, and CUDA 11.8+ are required",
        license_details: "MIT (Rosetta Commons). The RFdiffusion-Ab, ProteinMPNN, and RF2-Ab weights setup_system.sh downloads from files.ipd.uw.edu are public.",
        repo_url: Some("https://github.com/RosettaCommons/RFantibody"),
        home_url: None,
        docs_url: None,
        paper_url: Some("https://doi.org/10.1038/s41586-025-09721-5"),
        license: License::Mit,
        license_url: None,
    },
};
