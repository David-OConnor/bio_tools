use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::{
        Tool,
        catalog::{CatalogEntry, DataType, Identity, PrimaryInput},
    },
};

/// A whole saturation-mutagenesis scan in one forward pass, which is the
/// point of the method.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Alias {
        tool: Tool::ProteinMpnnDdg,
        slug: "proteinmpnn_ddg",
        name: None,
    },
    categories: &[ToolCategory::PropertyPrediction],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Moderate,
    primary_output: Some(DataType::Csv),
    primary_inputs: &[PrimaryInput::new("pdb_path", &[DataType::Pdb])],
    top_choice: true,
    spec: SpecData {
        summary: "Estimate changes in protein stability upon point mutation",
        description: "A modification of ProteinMPNN to use full sequence context. It introduces a decoding scheme \
        to improve computational efficiency and enable saturation mutagenesis studies at scale.",
        availability: "Installed by setup_system.sh with JAX for CPU or CUDA 12 on Linux/WSL",
        license_details: "MIT (Peptone), over MIT-licensed ProteinMPNN weights. Commercial use is unrestricted.",
        repo_url: Some("https://github.com/PeptoneLtd/proteinmpnn_ddg"),
        home_url: Some("https://peptone.io/"),
        docs_url: Some("https://github.com/PeptoneLtd/proteinmpnn_ddg#readme"),
        input_params_url: Some(
            "https://github.com/PeptoneLtd/proteinmpnn_ddg/blob/main/predict.py",
        ),
        examples_url: Some("https://github.com/PeptoneLtd/proteinmpnn_ddg/tree/main/example"),
        paper_url: Some("https://www.biorxiv.org/content/10.1101/2024.06.15.599145"),
        license: License::Mit,
        license_url: None,
        tested: true,
    },
};
