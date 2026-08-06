use super::{CatalogEntry, Identity};
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense, tool_definitions::Tool};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::RfAntibody),
    categories: &[
        ProcessCategory::AntibodyDesign,
        ProcessCategory::ProteinDesign,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: false,
    summary: "Design antibody or nanobody backbones against a target structure.",
    description: "Runs RFantibody's antibody-finetuned RFdiffusion stage with a target, framework, CDR loop ranges, and optional hotspots.",
    availability: "Linux, an NVIDIA GPU, RFantibody weights, and CUDA 11.8+ are required",
    license_details: "MIT (Rosetta Commons). The RFdiffusion-Ab, ProteinMPNN, and RF2-Ab weights setup_system.sh downloads from files.ipd.uw.edu are public.",
    repo_url: Some("https://github.com/RosettaCommons/RFantibody"),
    home_url: None,
    docs_url: None,
};
