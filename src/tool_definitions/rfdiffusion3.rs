use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::{
        Tool,
        catalog::{CatalogEntry, DataType, Identity, PrimaryInput},
    },
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
    primary_output: Some(DataType::MmCif),
    primary_inputs: &[PrimaryInput::new(
        "input",
        &[DataType::Pdb, DataType::MmCif],
    )],
    top_choice: true,
    spec: SpecData {
        summary: "Generates protein backbone coordinates around proteins, small molecules, \
         nucleic acids, and metals. Given geometric and other constraints, specifies backbone geometry. \
          A useful first step in a protein design pipeline.",
        description: "RFdiffusion3 is a practical first step in protein design workflows: It generates coordinates \
        of the backbone atoms for a protein based on contraints (For example, to spacially deconflict with specific \
        molecules). Its output can be fed into ProteinMPNN or LigandMPNN to generate an amino acid sequence.",
        availability: "Installed by setup_system.sh, which fetches the public model checkpoint; an NVIDIA GPU is required",
        license_details: "BSD 3-Clause from the Institute for Protein Design, University of Washington, covering the inference code, the training code, and the public checkpoint alike: unrestricted academic and commercial use.",
        repo_url: Some("https://github.com/RosettaCommons/foundry"),
        home_url: Some(
            "https://github.com/RosettaCommons/foundry/blob/production/models/rfd3/README.md",
        ),
        docs_url: Some("https://rosettacommons.github.io/foundry/models/rfd3/index.html"),
        input_params_url: Some(
            "https://rosettacommons.github.io/foundry/models/rfd3/input.html#inputspecification-fields",
        ),
        examples_url: Some(
            "https://github.com/RosettaCommons/foundry/tree/production/models/rfd3/docs/examples",
        ),
        paper_url: Some("https://doi.org/10.1101/2025.09.18.676967"),
        license: License::Bsd3Clause,
        license_url: None,
        tested: true,
    },
};
