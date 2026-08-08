use super::{CatalogEntry, Identity};
use crate::{
    LaunchType, License, LicenseCategory, ProcessExpense, SpecData, ToolCategory,
    tool_definitions::Tool,
};

/// The adapter runs pdb2gmx/editconf/solvate only. Preparing a system
/// takes seconds; it is the simulation this stops short of that would be
/// expensive.
pub const ENTRY: CatalogEntry = CatalogEntry {
    identity: Identity::Installed(Tool::Gromacs),
    categories: &[ToolCategory::MoleculeDynamics],
    launch_type: LaunchType::Executable,
    license_type: LicenseCategory::Copyleft,
    expense: ProcessExpense::Moderate,
    top_choice: false,
    spec: SpecData {
        summary: "A free and open-source software suite for high-performance molecular dynamics and output analysis.",
        description: "GROMACS is one of the most widely used open-source and free software codes in chemistry, used \
        primarily for dynamical simulations of biomolecules. It provides a rich set of calculation types, preparation \
        and analysis tools. Several advanced techniques for free-energy calculations are supported.",
        availability: "External GROMACS installation required for execution",
        license_details: "LGPL 2.1 or later. Free for commercial use, but distributing a modified GROMACS -- or anything statically linked against it -- carries the licence's reciprocal obligations. Invoking the binaries, which is all this adapter does, does not.",
        repo_url: Some("https://gitlab.com/gromacs/gromacs"),
        home_url: Some("https://www.gromacs.org/"),
        docs_url: Some("https://manual.gromacs.org/current/user-guide/flow.html"),
        paper_url: Some("https://doi.org/10.1016/j.softx.2015.06.001"),
        license: License::Lgpl21OrLater,
        license_url: None,
    },
};
