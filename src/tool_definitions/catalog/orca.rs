use super::{CatalogEntry, Identity};
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense};

/// Input-dependent over a wide range: a single point on a small molecule
/// is seconds, but the form also offers optimization and frequency jobs
/// across up to 32 cores.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Uninstalled {
        slug: "orca",
        name: "ORCA",
    },
    categories: &[ProcessCategory::QuantumChemistry],
    launch_type: LaunchType::Executable,
    license_type: LicenseType::Proprietary,
    expense: ProcessExpense::Expensive,
    top_choice: false,
    summary: "Build a single-point, optimization, or frequency calculation.",
    description: "Generates a conventional ORCA input deck and can run a configured local ORCA executable.",
    availability: "Manual ORCA installation required for execution",
    license_details: "Closed source, under the FAccTs end-user licence. Free for academic use after registration; commercial use requires a paid licence. The binaries are not redistributable, which is why setup_system.sh cannot fetch them.",
    repo_url: None,
    home_url: Some("https://www.faccts.de/orca/"),
    docs_url: Some("https://www.faccts.de/docs/orca/6.1/manual/"),
};
