use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Alias {
        tool: Tool::ProteinMpnn,
        slug: "abmpnn",
        name: Some("AbMPNN"),
    },
    categories: &[
        ToolCategory::AntibodyDesign,
        ToolCategory::SequencePrediction,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    spec: SpecData {
        summary: "Design antibody sequences from a backbone structure.",
        description: "Prepares an antibody-specific ProteinMPNN run using separately supplied AbMPNN weights.",
        availability: "ProteinMPNN checkout and separate AbMPNN model weights required",
        license_details: "Two licences, because this is ProteinMPNN's network run against someone else's weights: the ProteinMPNN code is MIT, and the AbMPNN weights are published by Exscientia on Zenodo under CC BY 4.0. Both allow commercial use with attribution.",
        repo_url: Some("https://github.com/dauparas/ProteinMPNN"),
        home_url: Some("https://zenodo.org/records/8164693"),
        docs_url: None,
        paper_url: Some("https://icml-compbio.github.io/2023/papers/WCBICML2023_paper61.pdf"),
        license: License::Mit,
        license_url: None,
    },
};
