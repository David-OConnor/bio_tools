use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::{
        Tool,
        catalog::{CatalogEntry, DataType, Identity, PrimaryInput},
    },
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::OpenDde),
    categories: &[ToolCategory::StructurePrediction],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    primary_output: Some(DataType::MmCif),
    primary_inputs: &[
        // "Set parameters here": the molecule boxes, which are a list of
        //   chains before this tool's adapter turns them into its own document.
        PrimaryInput::document(
            "sequence_molecules",
            &[
                DataType::AaSequence,
                DataType::DnaSequence,
                DataType::RnaSequence,
            ],
            "molecule_boxes",
        ),
        // "Enter YAML or JSON": the document itself.
        PrimaryInput::document(
            "input_json",
            &[
                DataType::AaSequence,
                DataType::DnaSequence,
                DataType::RnaSequence,
            ],
            "opendde_json",
        ),
    ],
    top_choice: true,
    spec: SpecData {
        summary: "Structure prediction for proteins, DNA/RNA, ligands, and ions. Supports co-folding.",
        description: "OpenDDE is an all-atom biomolecular foundation model that turns co-folding into a scalable engine for structure prediction, design, \
         and optimization in drug discovery. It models proteins, nucleic acids, and small molecules in one all-atom system.",
        availability: "Installed by setup_system.sh into its own uv environment; model weights download during installation",
        license_details: "Apache 2.0 (Aureka Research). Commercial use is permitted, with the licence's attribution and notice conditions.",
        repo_url: Some("https://github.com/aurekaresearch/OpenDDE"),
        home_url: Some("https://aurekaresearch.github.io/OpenDDE-Website/"),
        docs_url: Some("https://github.com/aurekaresearch/OpenDDE/tree/main/docs"),
        input_params_url: Some(
            "https://github.com/aurekaresearch/OpenDDE/blob/main/docs/infer_json_format.md",
        ),
        examples_url: Some("https://github.com/aurekaresearch/OpenDDE/tree/main/examples"),
        paper_url: Some("https://arxiv.org/abs/2607.03787"),
        license: License::ApacheV2,
        license_url: None,
        tested: true,
    },
};
