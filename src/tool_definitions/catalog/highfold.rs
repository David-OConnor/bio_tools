use super::{CatalogEntry, Identity};
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense, tool_definitions::Tool};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::HighFold),
    categories: &[
        ProcessCategory::StructurePrediction,
        ProcessCategory::PeptideBinderDesign,
    ],
    launch_type: LaunchType::CondaBasedApp,
    license_type: LicenseType::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: false,
    summary: "Predict the structure of a cyclic peptide or a cyclic-peptide complex.",
    description: "Runs HighFold, which folds cyclic peptides by feeding AlphaFold 2 a cyclic position offset matrix in place of its usual relative positional encoding, so head-to-tail and disulfide-bridged macrocycles are modelled as closed rather than as linear chains whose ends happen to meet.",
    availability: "Conda environment, an NVIDIA GPU, and the AlphaFold 2 parameters are required",
    license_details: "HighFold modifies ColabFold (MIT) and runs on the AlphaFold 2 parameters, which DeepMind publishes under CC BY 4.0. Both permit commercial use; confirm the upstream repository's own terms before relying on that.",
    repo_url: Some("https://github.com/hongliangduan/HighFold"),
    home_url: None,
    docs_url: Some("https://academic.oup.com/bib/article/25/3/bbae215/7665139"),
};
