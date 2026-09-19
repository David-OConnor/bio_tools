use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::{
        Tool,
        catalog::{CatalogEntry, DataType, Identity, PrimaryInput},
    },
};

/// A protein language model embedding per query, unlike DLKcat's much
/// smaller sequence CNN.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::CatPred),
    categories: &[
        ToolCategory::PropertyPrediction,
        ToolCategory::Cheminformatics,
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
        PrimaryInput::document("input_csv", &[DataType::AaSequence], "catpred_csv"),
    ],
    top_choice: false,
    spec: SpecData {
        summary: "Predict kcat, Km or Ki for an enzyme and its substrate, with an uncertainty estimate.",
        description: "CatPred combines an ESM-2 sequence embedding of the enzyme with a message-passing representation of the substrate, and predicts a distribution rather than a point value: each prediction carries total, aleatoric and epistemic standard deviations, and the epistemic part grows as the query moves away from the training data. One run scores one CSV of reactions -- enzyme sequence, substrate or inhibitor SMILES, and a sequence ID -- against one of the three parameters, using the ten-model production ensemble the authors predict with themselves.",
        availability: "Installed by bio_tools into its own uv environment, with the published checkpoint archive (about 10 GiB) downloaded beside the checkout. Predictions use data/pretrained/production/<parameter>, the ten-model ensembles behind CatPred's own demos and web app; the per-seed reproduce_checkpoints from the paper are used only if that directory is absent. Predictions run on CPU or CUDA, and each row costs one ESM-2 embedding (cached by sequence) plus ten model evaluations.",
        license_details: "MIT (Maranas group), weights included. Commercial use is unrestricted; confirm the repository's own licence before relying on that.",
        repo_url: Some("https://github.com/maranasgroup/CatPred"),
        home_url: Some("https://www.catpred.com/"),
        docs_url: Some("https://github.com/maranasgroup/CatPred#-prediction-"),
        input_params_url: Some(
            "https://github.com/maranasgroup/CatPred/blob/main/catpred/inference/types.py",
        ),
        examples_url: Some("https://github.com/maranasgroup/CatPred/tree/main/demo"),
        paper_url: Some("https://www.nature.com/articles/s41467-025-57215-9"),
        license: License::Mit,
        license_url: Some("https://github.com/maranasgroup/CatPred/blob/main/LICENSE"),
        tested: true,
    },
};
