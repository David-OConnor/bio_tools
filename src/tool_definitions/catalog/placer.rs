use super::CatalogEntry;
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense};

pub const ENTRY: CatalogEntry = CatalogEntry {
    slug: "placer",
    name: "PLACER",
    categories: &[
        ProcessCategory::StructurePrediction,
        ProcessCategory::Cheminformatics,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    summary: "Generate an ensemble of protein-ligand poses and side-chain conformations.",
    description: "Runs PLACER (Protein-Ligand Atomistic Conformational Ensemble Resolver), a graph network that denoises corrupted atomic coordinates back to plausible ones. Given a structure and approximate knowledge of the binding site, it samples an ensemble of ligand poses and side-chain conformations rather than a single answer, with a predicted uncertainty (prmsd) per sample.",
    availability: "Installed by setup_system.sh into its own uv environment; needs Linux, an NVIDIA GPU, and the checkout (weights are included in it)",
    license_details: "BSD 3-Clause (University of Washington, Institute for Protein Design), which the licence explicitly extends to the bundled model weights. Commercial use is unrestricted.",
    repo_url: Some("https://github.com/baker-laboratory/PLACER"),
    home_url: None,
    docs_url: Some("https://www.biorxiv.org/content/10.1101/2024.09.25.614868"),
};
