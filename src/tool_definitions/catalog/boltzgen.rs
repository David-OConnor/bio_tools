use super::CatalogEntry;
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense};

pub const ENTRY: CatalogEntry = CatalogEntry {
    slug: "boltzgen",
    name: "BoltzGen",
    categories: &[
        ProcessCategory::PeptideBinderDesign,
        ProcessCategory::ProteinDesign,
        ProcessCategory::AntibodyDesign,
    ],
    launch_type: LaunchType::PythonBasedApp,
    license_type: LicenseType::Permissive,
    expense: ProcessExpense::Expensive,
    top_choice: true,
    summary: "Design a protein, peptide, nanobody, or antibody binder.",
    description: "Builds a BoltzGen design specification YAML and runs the official boltzgen CLI end to end (design, inverse folding, and refolding).",
    availability: "Installed by setup_system.sh into its own uv environment; approximately 6 GB of model weights download separately",
    license_details: "MIT, covering the weights and training data as well as the inference code: unrestricted academic and commercial use.",
    repo_url: Some("https://github.com/HannesStark/boltzgen"),
    home_url: Some("https://boltz.bio/boltzgen"),
    docs_url: None,
};
