use super::CatalogEntry;
use crate::{LaunchType, LicenseType, ProcessCategory, ProcessExpense};

/// The adapter runs pdb2gmx/editconf/solvate only. Preparing a system
/// takes seconds; it is the simulation this stops short of that would be
/// expensive.
pub const ENTRY: CatalogEntry = CatalogEntry {
    slug: "gromacs",
    name: "GROMACS",
    categories: &[ProcessCategory::MoleculeDynamics],
    launch_type: LaunchType::Executable,
    license_type: LicenseType::Copyleft,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    summary: "Prepare a solvated molecular-dynamics system from a PDB structure.",
    description: "Runs the standard pdb2gmx, editconf, and optional solvate preparation steps.",
    availability: "External GROMACS installation required for execution",
    license_details: "LGPL 2.1 or later. Free for commercial use, but distributing a modified GROMACS -- or anything statically linked against it -- carries the licence's reciprocal obligations. Invoking the binaries, which is all this adapter does, does not.",
    repo_url: Some("https://gitlab.com/gromacs/gromacs"),
    home_url: Some("https://www.gromacs.org/"),
    docs_url: Some("https://manual.gromacs.org/current/user-guide/flow.html"),
};
