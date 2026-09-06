use crate::tool_definitions::catalog::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::LigandMpnn),
    categories: &[
        ToolCategory::ProteinDesign,
        ToolCategory::SequencePrediction,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    spec: SpecData {
        summary: "Protein sequence prediction, to conform with backbone coordinates. Takes external \
        molecules into account; to some degree a superset of ProteinMPNN, but is a different model. \
        A useful step after RFDiffusion in a protein design pipeline, and before validation \
        with structure prediction.",
        description: "A deep learning-based protein sequence design method that explicitly \
        models all non-protein components of biomolecular systems. \
        LigandMPNN generates not only sequences but also sidechain conformations to allow detailed \
        evaluation of binding interactions. Experimental characterization demonstrates that LigandMPNN can \
        generate small molecule and DNA-binding proteins with high affinity and specificity. \
        It allows explicit modeling of small molecule, nucleotide, metal, and other atomic contexts.",
        availability: "Installed by bio_tools with PyTorch, parser/packing dependencies and all official model variants; CPU or CUDA",
        license_details: "MIT, weights included. Commercial use is unrestricted.",
        repo_url: Some("https://github.com/dauparas/LigandMPNN"),
        home_url: None,
        docs_url: Some("https://github.com/dauparas/LigandMPNN#readme"),
        input_params_url: Some("https://github.com/dauparas/LigandMPNN/blob/main/run.py"),
        examples_url: Some("https://github.com/dauparas/LigandMPNN#design-examples"),
        paper_url: Some("https://www.biorxiv.org/content/10.1101/2023.12.22.573103v1"),
        license: License::Mit,
        license_url: None,
        tested: true,
    },
};
