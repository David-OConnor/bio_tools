//! The static, reusable catalog of every tool bio_tools describes: identity,
//! classification, and links -- independent of the [`Tool`](super::Tool)
//! install-recipe enum, which only covers tools with an unattended installer
//! and is keyed differently (see module docs there). A consuming application
//! supplies only what is genuinely its own: a numeric id for its own storage,
//! its adapter module, and UI field descriptors.

use crate::{
    LaunchType, LicenseCategory, ProcessExpense, Spec, SpecData, ToolCategory,
    tool_definitions::Tool,
};

pub mod fields;

mod abmpnn;
mod aggrescan3d;
mod alphafold3;
mod antibody_annotator;
mod antifold;
mod bindcraft;
mod biophi;
mod boltz2;
mod boltz_adme;
mod boltzgen;
mod catpred;
mod chai1;
mod deepimmuno;
mod deepsp;
mod deepstabp;
mod dlkcat;
mod esmfold2;
mod genie3;
mod germinal;
mod gromacs;
mod highfold;
mod igblast;
mod igdesign;
mod immunebuilder;
mod ligandmpnn;
mod mber;
mod netsolp;
mod opendde;
mod orca;
mod pdbbind;
mod placer;
mod proteinmpnn;
mod proteinmpnn_ddg;
mod protenix;
mod rdkit;
mod rfantibody;
mod rfdiffusion;
mod tap;
mod thermompnn;
mod tlimmuno;

/// How a catalog entry's slug and name relate to an install recipe in [`Tool`].
///
/// A slug or name that merely repeats what [`Tool::slug`]/[`Tool::name`] already say is not
/// stored again here; only genuinely distinct catalog data is.
#[derive(Debug, Clone, Copy)]
pub enum Identity {
    /// This entry is exactly the tool it installs: its slug and name come from `Tool`.
    Installed(Tool),
    /// This entry installs via `tool`, but is catalogued under its own slug and, optionally,
    /// its own name -- e.g. a distinct set of model weights run through the same script as
    /// another entry. A `name` of `None` falls back to `tool`'s.
    Alias {
        tool: Tool,
        slug: &'static str,
        name: Option<&'static str>,
    },
    /// No unattended install recipe exists for this entry.
    Uninstalled {
        slug: &'static str,
        name: &'static str,
    },
}

impl Identity {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Installed(tool) => tool.slug(),
            Self::Alias { slug, .. } | Self::Uninstalled { slug, .. } => slug,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Installed(tool) => tool.name(),
            Self::Alias {
                tool, name: None, ..
            } => tool.name(),
            Self::Alias {
                name: Some(name), ..
            }
            | Self::Uninstalled { name, .. } => name,
        }
    }

    /// The install recipe backing this entry, where one exists.
    pub const fn tool(self) -> Option<Tool> {
        match self {
            Self::Installed(tool) | Self::Alias { tool, .. } => Some(tool),
            Self::Uninstalled { .. } => None,
        }
    }
}

/// One catalog entry: everything about a tool that does not depend on the
/// consuming application.
#[derive(Debug, Clone, Copy)]
pub struct CatalogEntry {
    pub identity: Identity,
    pub categories: &'static [ToolCategory],
    pub launch_type: LaunchType,
    pub license_type: LicenseCategory,
    pub expense: ProcessExpense,
    pub top_choice: bool,
    pub spec: SpecData<&'static str>,
}

impl CatalogEntry {
    pub const fn slug(&self) -> &'static str {
        self.identity.slug()
    }

    pub const fn name(&self) -> &'static str {
        self.identity.name()
    }

    /// The subset of this entry consumed by [`crate::Process::spec`].
    pub fn to_spec(&self) -> Spec {
        Spec {
            slug: self.slug().to_owned(),
            data: self.spec.to_owned_data(),
        }
    }
}

/// Every catalog entry, in the order bio_web's catalog page groups them.
pub const ALL: &[&CatalogEntry] = &[
    &rdkit::ENTRY,
    &alphafold3::ENTRY,
    &opendde::ENTRY,
    &boltz2::ENTRY,
    &chai1::ENTRY,
    &protenix::ENTRY,
    &esmfold2::ENTRY,
    &immunebuilder::ENTRY,
    &highfold::ENTRY,
    &pdbbind::ENTRY,
    &boltzgen::ENTRY,
    &bindcraft::ENTRY,
    &gromacs::ENTRY,
    &orca::ENTRY,
    &igblast::ENTRY,
    &biophi::ENTRY,
    &antifold::ENTRY,
    &abmpnn::ENTRY,
    &proteinmpnn::ENTRY,
    &ligandmpnn::ENTRY,
    &proteinmpnn_ddg::ENTRY,
    &rfdiffusion::ENTRY,
    &rfantibody::ENTRY,
    &germinal::ENTRY,
    &mber::ENTRY,
    &igdesign::ENTRY,
    &thermompnn::ENTRY,
    &boltz_adme::ENTRY,
    &genie3::ENTRY,
    &deepsp::ENTRY,
    &deepimmuno::ENTRY,
    &tlimmuno::ENTRY,
    &netsolp::ENTRY,
    &deepstabp::ENTRY,
    &aggrescan3d::ENTRY,
    &dlkcat::ENTRY,
    &catpred::ENTRY,
    &antibody_annotator::ENTRY,
    &tap::ENTRY,
    &placer::ENTRY,
];

/// Look up a catalog entry by its slug (as used in `Spec.slug`, not
/// necessarily [`Tool::slug`](super::Tool::slug) -- a few entries share one
/// install recipe, or spell their slug differently, e.g. `"abmpnn"` and
/// `"proteinmpnn"` both install via `Tool::ProteinMpnn`.
pub fn by_slug(slug: &str) -> Option<&'static CatalogEntry> {
    ALL.iter().find(|entry| entry.slug() == slug).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_unique() {
        let mut slugs: Vec<&str> = ALL.iter().map(|entry| entry.slug()).collect();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), ALL.len());
    }

    #[test]
    fn by_slug_finds_every_entry() {
        for entry in ALL {
            assert_eq!(
                by_slug(entry.slug()).map(|found| found.slug()),
                Some(entry.slug())
            );
        }
    }
}
