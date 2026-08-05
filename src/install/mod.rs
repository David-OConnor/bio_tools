//! This manages downloading packages from official sources, and installing them. This may
//! involve running an install script.
//!
//! Each tool has its own file in this module.
//! Scripts are installed by executing using `std::process::command`. This is more verbose than using
//! shell scripts, but it's OS-agnostic and shell-agnostic, and integrates better with the rest
//! of the codebase.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

mod alphafold3;
mod boltz2;
mod boltzgen;
mod opendde;
mod protein_mpnn;
