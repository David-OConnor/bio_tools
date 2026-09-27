use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::{
        Tool,
        catalog::{CatalogEntry, DataType, Identity, PrimaryInput},
    },
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::AlphaFold3),
    categories: &[ToolCategory::StructurePrediction],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::NonCommercial,
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
        // "Enter JSON": the document itself, in the `alphafold3` dialect.
        PrimaryInput::document(
            "input_json",
            &[
                DataType::AaSequence,
                DataType::DnaSequence,
                DataType::RnaSequence,
            ],
            "alphafold3_json",
        ),
    ],
    top_choice: true,
    spec: SpecData {
        summary: "Structure prediction for proteins, DNA/RNA, ligands, ions, and modified \
        residues. Supports co-folding.",
        description: "Google DeepMind and Isomorphic Labs' AlphaFold 3 predicts the joint \
        all-atom structure of biomolecular complexes: proteins, DNA, RNA, ligands (by CCD code, \
        SMILES, or a user-provided CCD), ions, post-translational and nucleotide modifications, \
        glycans, and covalent bonds. Here, MSAs come from the ColabFold MMseqs2 server by \
        default, so the ~630 GB genetic databases are not needed; its own local data pipeline \
        is available for an operator who has installed them.",
        availability: "Linux and an NVIDIA GPU (compute capability 8.0 or newer; 7.x works with \
        XLA attention) are required. The code is built from the v3.0.4 release into its own uv \
        environment by setup_system.sh. The model parameters are separate: Google publishes \
        af3.bin.zst under the Model Parameters Terms of Use, and the operator either places it \
        in process_executables/alphafold3/models or sets ALPHAFOLD3_ACCEPT_WEIGHTS_TERMS=1 \
        before installing to have it fetched. Local genetic databases (optional) go in \
        process_executables/alphafold3/databases or ALPHAFOLD3_DATABASE_DIR.",
        license_details: "The source code is Apache 2.0, but the model parameters and every \
        prediction made with them are covered by the AlphaFold 3 Model Parameters Terms of Use: \
        only for non-commercial use by, or on behalf of, non-commercial organisations \
        (universities, non-profits, research institutes, education, journalism, government); \
        never in connection with commercial activities, including research on behalf of \
        commercial organisations; outputs may not be used to train structure-prediction \
        models; and the parameters may not be shared outside your organisation. Outputs are \
        subject to the AlphaFold 3 Output Terms of Use.",
        repo_url: Some("https://github.com/google-deepmind/alphafold3"),
        home_url: Some("https://deepmind.google/science/alphafold/"),
        docs_url: Some("https://github.com/google-deepmind/alphafold3/tree/v3.0.4/docs"),
        input_params_url: Some(
            "https://github.com/google-deepmind/alphafold3/blob/v3.0.4/docs/input.md",
        ),
        examples_url: Some("https://github.com/google-deepmind/alphafold3/tree/v3.0.4/examples"),
        paper_url: Some("https://doi.org/10.1038/s41586-024-07487-w"),
        license: License::Other,
        license_url: Some(
            "https://github.com/google-deepmind/alphafold3/blob/main/WEIGHTS_TERMS_OF_USE.md",
        ),
        tested: false,
    },
};
