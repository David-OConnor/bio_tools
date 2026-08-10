use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::ProteinMpnn),
    categories: &[
        ToolCategory::SequencePrediction,
        ToolCategory::ProteinDesign,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: true,
    spec: SpecData {
        summary: "Deep learning–based protein sequence design",
        description: "A graph neural network designed for protein inverse folding, meaning it predicts \
        the amino acid sequences most likely to fold into a specific 3D protein backbone structure. By \
        interpreting the spatial coordinates and geometric features of a target structure, the model \
        rapidly generates diverse and experimentally viable sequence candidates. It has become a standard \
        tool in computational protein engineering because it is significantly faster and more accurate \
        than older physics-based methods. Researchers widely use ProteinMPNN for applications such as \
        optimizing enzymes, designing novel therapeutics, and improving the stability or solubility of \
        synthetic proteins.",
        availability: "Installed by setup_system.sh with CUDA-enabled PyTorch for Linux/WSL and an NVIDIA GPU",
        license_details: "MIT, weights included. Commercial use is unrestricted.",
        repo_url: Some("https://github.com/dauparas/ProteinMPNN"),
        home_url: None,
        docs_url: None,
        // todo: or www.biorxiv.org/content/10.1101/2022.06.03.494563v1
        paper_url: Some("https://doi.org/10.1126/science.add2187"),
        license: License::Mit,
        license_url: None,
    },
};
