use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::{
        Tool,
        catalog::{CatalogEntry, DataType, Identity, PrimaryInput},
    },
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::EsmC),
    categories: &[
        ToolCategory::SequenceAnalysis,
        ToolCategory::PropertyPrediction,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
    primary_output: Some(DataType::Csv),
    primary_inputs: &[
        PrimaryInput::document(
            "sequence_molecules",
            &[DataType::AaSequence],
            "molecule_boxes",
        ),
        PrimaryInput::document("input_fasta", &[DataType::AaSequence], "esmc_fasta"),
    ],
    top_choice: false,
    spec: SpecData {
        summary: "Protein language-model embeddings, amino-acid probabilities, and substitution scores.",
        description: "ESM Cambrian analyzes protein sequences with local 300M, 600M, or 6B models. Exports per-residue and mean-pooled embeddings, optional hidden states and logits, position-level predictions, and optional masked-marginal single-substitution log-odds. Representations support downstream property prediction; scores are not calibrated measurements of stability, fitness, or function.",
        availability: "Installed into an isolated Python 3.12 environment using esm from PyPI; selected Biohub weights download from Hugging Face on first use. CPU and CUDA inference are supported; 6B requires substantial memory.",
        license_details: "The current Biohub ESMC code and released Biohub ESMC checkpoints use the MIT license.",
        repo_url: Some("https://github.com/Biohub/esm"),
        home_url: Some("https://biohub.ai/models/esmc"),
        docs_url: Some("https://github.com/Biohub/esm#running-esmc-through-hugging-face"),
        input_params_url: Some("https://github.com/Biohub/esm/blob/main/esm/models/esmc/model.py"),
        examples_url: Some(
            "https://github.com/Biohub/esm/blob/main/cookbook/tutorials/2_embed.ipynb",
        ),
        paper_url: Some("https://www.biorxiv.org/content/10.64898/2026.06.03.729735v1"),
        license: License::Mit,
        license_url: Some("https://github.com/Biohub/esm/blob/main/LICENSE"),
        tested: true,
    },
};
