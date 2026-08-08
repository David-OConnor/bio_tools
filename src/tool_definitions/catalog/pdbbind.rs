use super::{CatalogEntry, Identity};
use crate::{LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory};

/// A directory lookup against a dataset already on disk; nothing is computed.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Uninstalled {
        slug: "pdbbind",
        name: "PDBBind",
    },
    categories: &[ToolCategory::BindingData],
    launch_type: LaunchType::PythonLib,
    license_type: LicenseCategory::NonCommercial,
    expense: ProcessExpense::Cheap,
    top_choice: false,
    spec: SpecData {
        summary: "Locate a protein-ligand complex in a local PDBBind release.",
        description: "Checks a configured PDBBind dataset for a PDB entry and lists its structure and ligand files.",
        availability: "A separately licensed/downloaded dataset must be configured with PDBBIND_ROOT",
        license_details: "No licence is granted here: PDBbind+ is distributed by its maintainers under registration, free for academic use, with commercial use requiring a paid subscription. This adapter only reads a copy the operator has already obtained under their own agreement.",
        repo_url: None,
        home_url: Some("https://www.pdbbind-plus.org.cn/"),
        docs_url: None,
        paper_url: Some("https://doi.org/10.1021/jm030580l"),
        license: License::Other,
        license_url: None,
    },
};
