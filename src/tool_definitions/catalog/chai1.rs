use super::CatalogEntry;
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense};

pub const ENTRY: CatalogEntry = CatalogEntry {
    slug: "chai1",
    name: "Chai-1",
    categories: &[
        ProcessCategory::StructurePrediction,
        ProcessCategory::ProteinDesign,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: true,
    summary: "Predict protein or protein-ligand complex structures with Chai-1.",
    description: "Builds Chai's typed FASTA input and runs the official chai-lab folding CLI.",
    availability: "Linux and a CUDA GPU with bfloat16 support are required; model weights download on first use",
    license_details: "Apache 2.0 for both the code and the model weights; upstream states this covers commercial use including drug discovery. Earlier releases used the narrower Chai Discovery Community Licence.",
    repo_url: Some("https://github.com/chaidiscovery/chai-lab"),
    home_url: Some("https://www.chaidiscovery.com/"),
    docs_url: None,
};
