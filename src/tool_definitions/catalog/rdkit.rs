use super::{CatalogEntry, Identity};
use crate::{LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory};

pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Uninstalled {
        slug: "rdkit",
        name: "RDKit",
    },
    categories: &[ToolCategory::Cheminformatics],
    launch_type: LaunchType::PythonLib,
    license_type: LicenseCategory::Permissive,
    expense: ProcessExpense::Cheap,
    top_choice: false,
    spec: SpecData {
        summary: "Calculate common molecular descriptors and normalize SMILES.",
        description: "A lightweight, immediately runnable cheminformatics endpoint.",
        availability: "Included Python dependency",
        license_details: "BSD 3-Clause. Commercial use is unrestricted.",
        repo_url: Some("https://github.com/rdkit/rdkit"),
        home_url: Some("https://www.rdkit.org/"),
        docs_url: Some("https://www.rdkit.org/docs/GettingStartedInPython.html"),
        paper_url: None,
        license: License::Bsd3Clause,
        license_url: None,
    },
};
