use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::{
        Tool,
        catalog::{CatalogEntry, DataType, Identity, PrimaryInput},
    },
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::Protenix),
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
            "protenix_json",
        ),
    ],
    top_choice: true,
    spec: SpecData {
        summary: "Structure prediction for proteins, DNA/RNA, ligands, and ions. Supports co-folding.",
        description: "Protenix is ByteDance's trainable, open reproduction of AlphaFold 3. It predicts all-atom \
        structures of biomolecular complexes, including proteins, DNA, RNA, ligands, ions, and covalent modifications, \
        with optional MSA, template, RNA MSA, and pocket or contact constraint inputs.",
        availability: "Linux and a CUDA GPU are required. Installed by setup_system.sh into its own uv environment; \
        model weights download on first use.",
        license_details: "Apache 2.0 (ByteDance). Commercial use is permitted, with the licence's attribution and notice conditions.",
        repo_url: Some("https://github.com/bytedance/Protenix"),
        home_url: Some("https://protenix-server.com/"),
        docs_url: Some("https://github.com/bytedance/Protenix/tree/main/docs"),
        input_params_url: Some(
            "https://github.com/bytedance/Protenix/blob/main/docs/infer_json_format.md",
        ),
        examples_url: Some("https://github.com/bytedance/Protenix/tree/main/examples"),
        paper_url: Some("https://www.biorxiv.org/content/10.1101/2025.01.08.631967"),
        license: License::ApacheV2,
        license_url: None,
        tested: true,
    },
};
