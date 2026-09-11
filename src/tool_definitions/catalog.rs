//! The static, reusable catalog of every tool bio_tools describes: identity,
//! classification, and links -- independent of the [`Tool`](super::Tool)
//! install-recipe enum, which only covers tools with an unattended installer
//! and is keyed differently (see module docs there). A consuming application
//! supplies only what is genuinely its own: a numeric id for its own storage,
//! its adapter module, and UI field descriptors.

use std::fmt;

use crate::{
    LaunchType, LicenseCategory, ProcessExpense, Spec, SpecData, ToolCategory,
    tool_definitions::{
        Tool, abmpnn, aggrescan3d, alphafold3, antibody_annotator, antifold, bindcraft, biophi,
        boltz_adme, boltz2, boltzgen, catpred, chai1, deepimmuno, deepsp, deepstabp, dlkcat,
        enzymemap, esmfold2, genie3, germinal, gromacs, highfold, igblast, igdesign, immunebuilder,
        ligandmpnn, mber, netsolp, opendde, orca, pdbbind, placer, proteinmpnn, proteinmpnn_ddg,
        protenix, rdkit, retrobiocat, rfantibody, rfdiffusion3, tap, thermompnn, tlimmuno,
    },
};

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

/// What a file handed between tools is made of.
///
/// Named for the content rather than for the file name, because the two do not
/// agree: a set of coordinates is an `.mmcif`, a `.cif`, a `.pdb` or an `.ent`,
/// and any of the four may arrive gzipped. [`DataType::category`] is the
/// grouping a consumer reads a directory of output with; the variants
/// themselves are what an input field is matched against, since a tool that
/// only parses PDB cannot be handed an mmCIF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataType {
    MmCif,
    Pdb,
    AaSequence,
    DnaSequence,
    RnaSequence,
    /// A table of scores or predictions, one row per thing scored.
    Csv,
}

/// The family of [`DataType`], which is what finding the file comes down to.
///
/// A run leaves a directory, and picking its headline file out means knowing
/// which suffixes to look for and what to call what is found -- neither of
/// which distinguishes an mmCIF from a PDB, or DNA from protein.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataCategory {
    Structure,
    Sequence,
    Table,
}

impl DataCategory {
    /// A word for a job row, where there is room for one: "Structure", "Seq".
    pub const fn label(self) -> &'static str {
        match self {
            Self::Structure => "Structure",
            Self::Sequence => "Seq",
            Self::Table => "CSV",
        }
    }

    /// Every file suffix this family is written with, lower-case and without
    /// any `.gz` a tool may have added on top.
    pub const fn suffixes(self) -> &'static [&'static str] {
        match self {
            Self::Structure => &[".cif", ".mmcif", ".pdb", ".ent"],
            Self::Sequence => &[".fa", ".fasta", ".faa", ".fas"],
            Self::Table => &[".csv"],
        }
    }
}

impl fmt::Display for DataCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Structure => "Structure",
            Self::Sequence => "Sequence",
            Self::Table => "Table",
        };
        write!(f, "{s}")
    }
}

impl DataType {
    pub const fn category(self) -> DataCategory {
        match self {
            Self::MmCif | Self::Pdb => DataCategory::Structure,
            Self::AaSequence | Self::DnaSequence | Self::RnaSequence => DataCategory::Sequence,
            Self::Csv => DataCategory::Table,
        }
    }

    /// Whether a file of this type can be fed to an input that wants `wanted`.
    ///
    /// Exact for now, and deliberately so: the near misses are real conversions
    /// -- mmCIF to PDB truncates chain names and residue numbers -- rather than
    /// things a caller can do by copying bytes.
    pub const fn feeds(self, wanted: Self) -> bool {
        matches!(
            (self, wanted),
            (Self::MmCif, Self::MmCif)
                | (Self::Pdb, Self::Pdb)
                | (Self::AaSequence, Self::AaSequence)
                | (Self::DnaSequence, Self::DnaSequence)
                | (Self::RnaSequence, Self::RnaSequence)
                | (Self::Csv, Self::Csv)
        )
    }

    /// The stable name a consuming application stores and compares against.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MmCif => "MmCif",
            Self::Pdb => "Pdb",
            Self::AaSequence => "AaSequence",
            Self::DnaSequence => "DnaSequence",
            Self::RnaSequence => "RnaSequence",
            Self::Csv => "Csv",
        }
    }
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::MmCif => "mmCIF",
            Self::Pdb => "PDB",
            Self::AaSequence => "Amino acid sequence",
            Self::DnaSequence => "DNA sequence",
            Self::RnaSequence => "RNA sequence",
            Self::Csv => "CSV",
        };
        write!(f, "{s}")
    }
}

/// How a chosen file's contents reach the field they are dropped into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputForm {
    /// The file exactly as its producer wrote it, headers and all.
    File,
    /// Residues alone, one chain per line, with FASTA headers dropped. What a
    /// field asking for "one sequence per line" wants.
    Residues,
    /// The field holds a whole input document naming several entities -- the
    /// structure predictors take one of these rather than a bare sequence --
    /// and a chosen sequence joins it as one more chain rather than replacing
    /// it. The name is the document's dialect, which the consuming application
    /// is the one that knows how to write.
    Document(&'static str),
}

impl InputForm {
    /// Which of the three this is, as the name a consumer switches on.
    pub const fn kind(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Residues => "residues",
            Self::Document(_) => "document",
        }
    }

    /// The document dialect, for [`Self::Document`] and nothing else.
    pub const fn dialect(self) -> Option<&'static str> {
        match self {
            Self::Document(dialect) => Some(dialect),
            _ => None,
        }
    }
}

/// The one input a tool is chiefly given, where it has one.
///
/// This is what makes chaining possible: it names the form field a previous
/// run's output goes into, and what that field will accept there. A tool that
/// takes nothing a previous run could produce -- a SMILES string, a pair of
/// antibody chains with no way to say which is which -- leaves this unset.
#[derive(Debug, Clone, Copy)]
pub struct PrimaryInput {
    /// The field, by the `name` it carries in this tool's field descriptors.
    pub field: &'static str,
    pub accepts: &'static [DataType],
    pub form: InputForm,
}

impl PrimaryInput {
    pub const fn new(field: &'static str, accepts: &'static [DataType]) -> Self {
        Self {
            field,
            accepts,
            form: InputForm::File,
        }
    }

    /// An input wanting bare residues rather than the FASTA file around them.
    pub const fn residues(field: &'static str, accepts: &'static [DataType]) -> Self {
        Self {
            field,
            accepts,
            form: InputForm::Residues,
        }
    }

    /// An input holding a whole document, which a chosen sequence joins.
    pub const fn document(
        field: &'static str,
        accepts: &'static [DataType],
        dialect: &'static str,
    ) -> Self {
        Self {
            field,
            accepts,
            form: InputForm::Document(dialect),
        }
    }

    /// Whether this input can be handed a file of `produced`.
    pub fn accepts(&self, produced: DataType) -> bool {
        self.accepts.iter().any(|wanted| produced.feeds(*wanted))
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
    /// The one file this tool exists to produce, offered as a download of its
    /// own and as the input to whatever runs next. Unset for a tool whose
    /// output is a JSON answer rather than a file.
    pub primary_output: Option<DataType>,
    /// Where a previous run's output can be dropped into this tool's form.
    /// More than one where the tool offers a choice of input modes, since each
    /// mode has a field of its own; empty for a tool nothing can be fed to.
    pub primary_inputs: &'static [PrimaryInput],
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
    &enzymemap::ENTRY,
    &retrobiocat::ENTRY,
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
    &rfdiffusion3::ENTRY,
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
