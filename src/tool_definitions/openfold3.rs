use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::{
        Tool,
        catalog::{CatalogEntry, DataType, Identity, PrimaryInput},
    },
};

const SEQUENCES: &[DataType] = &[
    DataType::AaSequence,
    DataType::DnaSequence,
    DataType::RnaSequence,
];

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::OpenFold3),
    categories: &[ToolCategory::StructurePrediction],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    primary_output: Some(DataType::MmCif),
    primary_inputs: &[
        PrimaryInput::document("sequence_molecules", SEQUENCES, "molecule_boxes"),
        PrimaryInput::document("input_json", SEQUENCES, "openfold3_json"),
    ],
    top_choice: true,
    spec: SpecData {
        summary: "Structure prediction for proteins, DNA/RNA, and noncovalent ligands.",
        description: "OpenFold3 runs the OpenBind-0 checkpoint for all-atom prediction of \
        biomolecular complexes. Protein MSAs can come from ColabFold or precomputed files. \
        The ColabFold mode sends protein sequences to the configured MSA server. Set \
        OPENFOLD3_MSA_SERVER_URL on the runner to use a private ColabFold server.",
        availability: "The managed installation uses Linux and NVIDIA CUDA. It installs \
        OpenFold3 0.5.1 and caches the approximately 2 GB OpenBind-0 checkpoint. No local \
        sequence databases are required.",
        license_details: "Apache 2.0 for both the OpenFold3 code and the OpenBind-0 checkpoint.",
        repo_url: Some("https://github.com/aqlaboratory/openfold-3"),
        home_url: Some("https://portal.openfold.omsf.io/"),
        docs_url: Some("https://openfold-3.readthedocs.io/en/latest/inference.html"),
        input_params_url: Some(
            "https://openfold-3.readthedocs.io/en/latest/input_format_reference.html",
        ),
        examples_url: Some(
            "https://github.com/aqlaboratory/openfold-3/tree/0.5.1/examples/example_inference_inputs",
        ),
        paper_url: None,
        license: License::ApacheV2,
        license_url: Some("https://openbind.uk/documents-and-tools/"),
        tested: true,
    },
};
