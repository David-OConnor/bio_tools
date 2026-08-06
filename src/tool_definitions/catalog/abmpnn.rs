use super::CatalogEntry;
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense};

pub const ENTRY: CatalogEntry = CatalogEntry {
    slug: "abmpnn",
    name: "AbMPNN",
    categories: &[
        ProcessCategory::AntibodyDesign,
        ProcessCategory::SequencePrediction,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    summary: "Design antibody sequences from a backbone structure.",
    description: "Prepares an antibody-specific ProteinMPNN run using separately supplied AbMPNN weights.",
    availability: "ProteinMPNN checkout and separate AbMPNN model weights required",
    license_details: "Two licences, because this is ProteinMPNN's network run against someone else's weights: the ProteinMPNN code is MIT, and the AbMPNN weights are published by Exscientia on Zenodo under CC BY 4.0. Both allow commercial use with attribution.",
    repo_url: Some("https://github.com/dauparas/ProteinMPNN"),
    home_url: Some("https://zenodo.org/records/8164693"),
    docs_url: None,
};
