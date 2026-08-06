use super::{CatalogEntry, Identity};
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense, tool_definitions::Tool};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::Germinal),
    categories: &[
        ProcessCategory::AntibodyDesign,
        ProcessCategory::ProteinDesign,
        ProcessCategory::PeptideBinderDesign,
    ],
    launch_type: LaunchType::CondaBasedApp,
    license_type: LicenseType::NonCommercial,
    expense: ProcessExpense::Expensive,
    top_choice: true,
    summary: "Design de novo antibodies against a chosen epitope on a target structure.",
    description: "Runs the Germinal pipeline, which hallucinates VHH or scFv binders through AlphaFold-Multimer under antibody language model guidance and then filters the trajectories on its structural and sequence criteria. Germinal's own configuration is a set of named Hydra profiles rather than a flat set of flags; this writes a target profile for the submitted structure and selects the matching run and filter profiles for the chosen binder format.",
    availability: "Conda environment, an NVIDIA GPU, PyRosetta, and the AlphaFold 2 parameters are required",
    license_details: "MIT code, but the pipeline as it runs is narrower than that: PyRosetta needs a separate licence for commercial use, and the IgLM weights that guide design are released for non-commercial research only.",
    repo_url: Some("https://github.com/SantiagoMille/germinal"),
    home_url: None,
    docs_url: Some("https://www.biorxiv.org/content/10.1101/2025.09.19.677421v1"),
};
