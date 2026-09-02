use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::RfDiffusion3),
    categories: &[
        ToolCategory::ProteinDesign,
        ToolCategory::BackboneGeneration,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: true,
    spec: SpecData {
        summary: "Generates all-atom protein backbone coordinates around proteins, small molecules, \
         nucleic acids, and metals. A useful first step in a protein design pipeline.",
        description: "RFdiffusion3 is a practical first step in protein design workflows: It generates coordinates \
        of the backbone atoms for a protein based on contraints (For example, to spacially deconflict with specific \
        molecules). Its output can be fed into ProteinMPNN or LigandMPNN to generate an amino acid sequence.",
        availability: "Installed by setup_system.sh, which fetches the public model checkpoint; an NVIDIA GPU is required",
        license_details: "BSD 3-Clause from the Institute for Protein Design, University of Washington, covering the inference code, the training code, and the public checkpoint alike: unrestricted academic and commercial use.",
        repo_url: Some("https://github.com/RosettaCommons/foundry"),
        home_url: Some("https://rosettacommons.github.io/foundry/models/rfd3/index.html"),
        docs_url: Some("https://rosettacommons.github.io/foundry/models/rfd3/input.html"),
        paper_url: Some("https://doi.org/10.1101/2025.09.18.676967"),
        license: License::Bsd3Clause,
        license_url: None,
    },
};
