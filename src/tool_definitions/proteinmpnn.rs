use crate::tool_definitions::catalog::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::ProteinMpnn),
    categories: &[
        ToolCategory::ProteinDesign,
        ToolCategory::SequencePrediction,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: true,
    spec: SpecData {
        summary: "Protein sequence prediction, to conform with backbone coordinates. Does not take external \
        molecules into account. A useful step after RFDiffusion in a protein design pipeline, and before validation \
        with structure prediction.",
        description: "A graph neural network designed for protein inverse folding, meaning it predicts \
        the amino acid sequences most likely to fold into a specific 3D protein backbone structure. By \
        interpreting the spatial coordinates and geometric features of a target structure, the model \
        generates sequence candidates. Researchers use ProteinMPNN for applications such as \
        optimizing enzymes, designing novel therapeutics, and improving the stability or solubility of \
        synthetic proteins.",
        availability: "Installed by bio_tools with PyTorch and the official vanilla, soluble and CA-only checkpoints; CPU or CUDA",
        license_details: "MIT, weights included. Commercial use is unrestricted.",
        repo_url: Some("https://github.com/dauparas/ProteinMPNN"),
        home_url: None,
        docs_url: Some("https://github.com/dauparas/ProteinMPNN#readme"),
        // "Input flags" section.
        input_params_url: Some("https://github.com/dauparas/ProteinMPNN/blob/main/README.md"),
        examples_url: Some("https://github.com/dauparas/ProteinMPNN/tree/main/examples"),
        // todo: or www.biorxiv.org/content/10.1101/2022.06.03.494563v1
        paper_url: Some("https://doi.org/10.1126/science.add2187"),
        license: License::Mit,
        license_url: None,
    },
};
