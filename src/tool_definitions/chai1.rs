use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::{
        Tool,
        catalog::{CatalogEntry, DataType, Identity, PrimaryInput},
    },
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::Chai1),
    categories: &[ToolCategory::StructurePrediction],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    primary_output: Some(DataType::MmCif),
    primary_inputs: &[
        PrimaryInput::document(
            "sequence_molecules",
            &[
                DataType::AaSequence,
                DataType::DnaSequence,
                DataType::RnaSequence,
            ],
            "molecule_boxes",
        ),
        PrimaryInput::document(
            "input_fasta",
            &[
                DataType::AaSequence,
                DataType::DnaSequence,
                DataType::RnaSequence,
            ],
            "chai_fasta",
        ),
    ],
    top_choice: true,
    spec: SpecData {
        summary: "Molecular structure prediction, including proteins. Similar to AlphaFold3.",
        description: "A multi-modal foundation model for molecular structure prediction that performs at \
        the state-of-the-art across a variety of benchmarks. Chai-1 enables unified prediction of proteins, \
        small molecules, DNA, RNA, glycosylations, and more.",
        availability: "Linux and a CUDA GPU with bfloat16 support are required; model weights download on first use",
        license_details: "Apache 2.0 for both the code and the model weights; upstream states this covers commercial use including drug discovery. Earlier releases used the narrower Chai Discovery Community Licence.",
        repo_url: Some("https://github.com/chaidiscovery/chai-lab"),
        home_url: Some("https://www.chaidiscovery.com/"),
        docs_url: Some("https://github.com/chaidiscovery/chai-lab/tree/main/examples"),
        input_params_url: Some(
            "https://github.com/chaidiscovery/chai-lab/blob/main/chai_lab/chai1.py#L482",
        ),
        examples_url: Some("https://github.com/chaidiscovery/chai-lab/tree/main/examples"),
        paper_url: Some("https://www.biorxiv.org/content/10.1101/2024.10.10.615955v2"),
        license: License::ApacheV2,
        license_url: Some("https://github.com/chaidiscovery/chai-lab/blob/main/LICENSE"),
        tested: true,
    },
};
