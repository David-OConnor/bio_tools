use super::{CatalogEntry, Identity};
use crate::{LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory};

/// Input-dependent over a wide range: a single point on a small molecule
/// is seconds, but the form also offers optimization and frequency jobs
/// across up to 32 cores.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Uninstalled {
        slug: "orca",
        name: "ORCA",
    },
    categories: &[ToolCategory::QuantumChemistry],
    launch_type: LaunchType::Executable,
    license_type: LicenseCategory::Proprietary,
    expense: ProcessExpense::Expensive,
    top_choice: false,
    spec: SpecData {
        summary: "A powerful and versatile quantum chemistry software package",
        description: "ORCA is a multi-purpose quantum chemistry software package. It features a wide variety of methods \
        ranging from semi-empirical methods to density functional theory to correlated single- and multi-reference wave \
        function-based methods. Environmental as well as relativistic effects can be taken into account.",
        availability: "Manual ORCA installation required for execution",
        license_details: "Closed source, under the FAccTs end-user licence. Free for academic use after registration; commercial use requires a paid licence. The binaries are not redistributable, which is why setup_system.sh cannot fetch them.",
        repo_url: None,
        home_url: Some("https://www.faccts.de/orca/"),
        docs_url: Some("https://www.faccts.de/docs/orca/6.1/manual/"),
        paper_url: Some("https://doi.org/10.1002/wcms.81"),
        license: License::Other,
        license_url: None,
    },
};
