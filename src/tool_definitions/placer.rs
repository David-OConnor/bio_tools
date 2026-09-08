use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::{
        Tool,
        catalog::{CatalogEntry, Identity},
    },
};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::Placer),
    categories: &[
        ToolCategory::ProteinDesign,
        ToolCategory::StructurePrediction,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    spec: SpecData {
        summary: "Generate an ensemble of protein-ligand poses and side-chain conformations. Given a \
        protein pocket and ligand, estimates how the ligand and sidechains will arrange themselves.",
        description: "Runs PLACER (Protein-Ligand Atomistic Conformational Ensemble Resolver), a graph network that denoises corrupted atomic coordinates back to plausible ones. Given a structure and approximate knowledge of the binding site, it samples an ensemble of ligand poses and side-chain conformations rather than a single answer, with a predicted uncertainty (prmsd) per sample.",
        availability: "Installed by setup_system.sh into its own uv environment; needs Linux, an NVIDIA GPU, and the checkout (weights are included in it)",
        license_details: "BSD 3-Clause (University of Washington, Institute for Protein Design), which the licence explicitly extends to the bundled model weights. Commercial use is unrestricted.",
        repo_url: Some("https://github.com/baker-laboratory/PLACER"),
        home_url: None,
        docs_url: Some("https://www.biorxiv.org/content/10.1101/2024.09.25.614868"),
        input_params_url: None,
        examples_url: None,
        paper_url: Some("https://www.biorxiv.org/content/10.1101/2024.09.25.614868"),
        license: License::Bsd3Clause,
        license_url: None,
        tested: false,
    },
};
