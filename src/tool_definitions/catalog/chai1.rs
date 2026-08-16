use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::Chai1),
    categories: &[
        ToolCategory::StructurePrediction,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Expensive,
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
        docs_url: None,
        paper_url: Some("https://www.biorxiv.org/content/10.1101/2024.10.10.615955"),
        license: License::ApacheV2,
        license_url: None,
    },
};
