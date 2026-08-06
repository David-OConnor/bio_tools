//! Individual tool details, which conform to our interfaces.

use std::fmt;
use std::str::FromStr;
use crate::install::{InstallError, Installer};

/// A tool with an unattended or partially unattended installation recipe.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Tool {
    AlphaFold3,
    OpenDde,
    Boltz2,
    Chai1,
    Protenix,
    EsmFold2,
    ImmuneBuilder,
    HighFold,
    BoltzGen,
    BindCraft,
    IgBlast,
    BioPhi,
    AntiFold,
    ProteinMpnn,
    LigandMpnn,
    ProteinMpnnDdg,
    RfDiffusion,
    RfAntibody,
    Germinal,
    Mber,
    IgDesign,
    ThermoMpnn,
    Genie3,
    DeepSp,
    DeepImmuno,
    TlImmuno2,
    NetSolP,
    DeepStabP,
    AggreScan3d,
    DlkCat,
    CatPred,
    Anarcii,
    Placer,
    Gromacs,
    BoltzAdme,
}

impl Tool {
    /// Every recipe in a stable, user-facing order.
    pub const ALL: [Self; 35] = [
        Self::AlphaFold3,
        Self::OpenDde,
        Self::Boltz2,
        Self::Chai1,
        Self::Protenix,
        Self::EsmFold2,
        Self::ImmuneBuilder,
        Self::HighFold,
        Self::BoltzGen,
        Self::BindCraft,
        Self::LigandMpnn,
        Self::ProteinMpnn,
        Self::ProteinMpnnDdg,
        Self::RfDiffusion,
        Self::RfAntibody,
        Self::Germinal,
        Self::Mber,
        Self::IgDesign,
        Self::ThermoMpnn,
        Self::Genie3,
        Self::DeepSp,
        Self::DeepImmuno,
        Self::TlImmuno2,
        Self::NetSolP,
        Self::DeepStabP,
        Self::AggreScan3d,
        Self::DlkCat,
        Self::CatPred,
        Self::IgBlast,
        Self::BioPhi,
        Self::Anarcii,
        Self::Placer,
        Self::Gromacs,
        Self::BoltzAdme,
        Self::AntiFold,
    ];

    /// Stable machine-readable name used for environment and checkout paths.
    pub const fn slug(self) -> &'static str {
        match self {
            Self::AlphaFold3 => "alphafold3",
            Self::OpenDde => "opendde",
            Self::Boltz2 => "boltz2",
            Self::Chai1 => "chai1",
            Self::Protenix => "protenix",
            Self::EsmFold2 => "esmfold2",
            Self::ImmuneBuilder => "immunebuilder",
            Self::HighFold => "highfold",
            Self::BoltzGen => "boltzgen",
            Self::BindCraft => "bindcraft",
            Self::IgBlast => "igblast",
            Self::BioPhi => "biophi",
            Self::AntiFold => "antifold",
            Self::ProteinMpnn => "proteinmpnn",
            Self::LigandMpnn => "ligandmpnn",
            Self::ProteinMpnnDdg => "proteinmpnn-ddg",
            Self::RfDiffusion => "rfdiffusion",
            Self::RfAntibody => "rfantibody",
            Self::Germinal => "germinal",
            Self::Mber => "mber",
            Self::IgDesign => "igdesign",
            Self::ThermoMpnn => "thermompnn",
            Self::Genie3 => "genie3",
            Self::DeepSp => "deepsp",
            Self::DeepImmuno => "deepimmuno",
            Self::TlImmuno2 => "tlimmuno2",
            Self::NetSolP => "netsolp",
            Self::DeepStabP => "deepstabp",
            Self::AggreScan3d => "aggrescan3d",
            Self::DlkCat => "dlkcat",
            Self::CatPred => "catpred",
            Self::Anarcii => "anarcii",
            Self::Placer => "placer",
            Self::Gromacs => "gromacs",
            Self::BoltzAdme => "boltz-adme",
        }
    }

    /// The console script the tool's environment exposes, for the tools that publish one.
    ///
    /// Upstream packages usually name their entry point after the slug, so that is the default;
    /// the arms below are the packages whose executable is named after something else, such as the
    /// distribution (`chai-lab`) or a single supported model (`ABodyBuilder2`, `mber-vhh`).
    pub const fn console_script(self) -> &'static str {
        match self {
            Self::Boltz2 => "boltz",
            Self::Chai1 => "chai-lab",
            Self::EsmFold2 => "esm-fold",
            Self::ImmuneBuilder => "ABodyBuilder2",
            Self::AggreScan3d => "aggrescan",
            Self::Mber => "mber-vhh",
            _ => self.slug(),
        }
    }

    /// Human-readable upstream name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::AlphaFold3 => "AlphaFold 3",
            Self::OpenDde => "OpenDDE",
            Self::Boltz2 => "Boltz-2",
            Self::Chai1 => "Chai-1",
            Self::Protenix => "Protenix-v2",
            Self::EsmFold2 => "ESMFold 2",
            Self::ImmuneBuilder => "ImmuneBuilder",
            Self::HighFold => "HighFold",
            Self::BoltzGen => "BoltzGen",
            Self::BindCraft => "BindCraft",
            Self::IgBlast => "IgBLAST",
            Self::BioPhi => "BioPhi",
            Self::AntiFold => "AntiFold",
            Self::ProteinMpnn => "ProteinMPNN",
            Self::LigandMpnn => "LigandMPNN",
            Self::ProteinMpnnDdg => "ProteinMPNN-ddG",
            Self::RfDiffusion => "RFdiffusion",
            Self::RfAntibody => "RFantibody",
            Self::Germinal => "Germinal",
            Self::Mber => "mBER",
            Self::IgDesign => "IgDesign",
            Self::ThermoMpnn => "ThermoMPNN",
            Self::Genie3 => "Genie 3",
            Self::DeepSp => "DeepSP",
            Self::DeepImmuno => "DeepImmuno",
            Self::TlImmuno2 => "TLimmuno2",
            Self::NetSolP => "NetSolP",
            Self::DeepStabP => "DeepSTABp",
            Self::AggreScan3d => "AggreScan3D",
            Self::DlkCat => "DLKcat",
            Self::CatPred => "CatPred",
            Self::Anarcii => "ANARCII",
            Self::Placer => "PLACER",
            Self::Gromacs => "GROMACS",
            Self::BoltzAdme => "Boltz ADME",
        }
    }

    /// Directories this tool's recipe creates under the tools root, relative to it.
    ///
    /// The isolated Python or micromamba environment is not listed: every recipe has one, and it
    /// is derived from the slug. This is only the checkouts, binary distributions, and model
    /// assets, which is what [`Installer::uninstall`] needs and no probe can rediscover.
    pub const fn asset_directories(self) -> &'static [&'static str] {
        match self {
            Self::HighFold => &["HighFold"],
            Self::BindCraft => &["BindCraft"],
            Self::IgBlast => &["igblast"],
            Self::AntiFold => &["AntiFold"],
            Self::ProteinMpnn => &["ProteinMPNN"],
            Self::LigandMpnn => &["LigandMPNN"],
            Self::RfDiffusion => &["RFdiffusion"],
            Self::RfAntibody => &["RFantibody"],
            Self::Germinal => &["germinal"],
            Self::Mber => &["mber-open"],
            Self::IgDesign => &["igdesign"],
            Self::ThermoMpnn => &["ThermoMPNN"],
            Self::Genie3 => &["genie3"],
            Self::DeepSp => &["DeepSP"],
            Self::DeepImmuno => &["DeepImmuno"],
            Self::TlImmuno2 => &["TLimmuno2"],
            Self::NetSolP => &["NetSolP-1.0"],
            Self::DeepStabP => &["deepStabP"],
            Self::DlkCat => &["DLKcat"],
            Self::CatPred => &["CatPred"],
            Self::Placer => &["PLACER"],
            Self::Gromacs => &["gromacs"],
            _ => &[],
        }
    }

    /// The named Conda environment an upstream installer creates, where one does.
    ///
    /// Everything else gets a prefix environment under the layout's environments root, which is
    /// an ordinary directory. These three are in Conda's own envs directory and only Conda knows
    /// where that is.
    pub const fn conda_environment(self) -> Option<&'static str> {
        match self {
            Self::BindCraft => Some("BindCraft"),
            Self::Genie3 => Some("genie3"),
            // The micromamba branch of the recipe builds a prefix environment instead; removing a
            // name that was never created is harmless and covers the install.sh branch.
            Self::Germinal => Some("germinal"),
            _ => None,
        }
    }

    /// Assets an uninstall leaves in place, and why, for tools that have any.
    pub const fn retained_assets(self) -> &'static [&'static str] {
        match self {
            Self::HighFold => &[
                "The AlphaFold 2 parameters under alphafold_params were kept: they are several \
                 gigabytes, are not specific to this tool, and are reused by any recipe that \
                 needs them.",
            ],
            Self::OpenDde => &[
                "The OpenDDE model checkpoint under ~/.cache/opendde was kept: it lives outside \
                 the managed tree and is reused by a later reinstall.",
            ],
            Self::AlphaFold3 => &[
                "The AlphaFold 3 model parameters and genetic databases were kept: they are \
                 licensed assets configured by hand, not installed by this recipe.",
            ],
            _ => &[],
        }
    }

    /// Whether upstream publishes the required binaries and wheels for the current platform.
    pub const fn is_supported(self) -> bool {
        cfg!(target_os = "linux")
            || !matches!(
                self,
                Self::AlphaFold3
                    | Self::Chai1
                    | Self::Protenix
                    | Self::EsmFold2
                    | Self::HighFold
                    | Self::BoltzGen
                    | Self::BindCraft
                    | Self::AntiFold
                    | Self::ProteinMpnnDdg
                    | Self::RfDiffusion
                    | Self::RfAntibody
                    | Self::Germinal
                    | Self::Mber
                    | Self::IgDesign
                    | Self::Genie3
                    | Self::AggreScan3d
                    | Self::CatPred
                    | Self::Placer
                    | Self::Gromacs
            )
    }

    /// Recipes supported on the current operating system.
    pub fn supported() -> impl Iterator<Item = Self> {
        Self::ALL.into_iter().filter(|tool| tool.is_supported())
    }
}

impl fmt::Display for Tool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Tool {
    type Err = InstallError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        // Consumer registries use a mixture of display names, executable names, hyphens, and
        // underscores. Treat punctuation as presentation so `Process::install` can resolve either
        // form without every application maintaining another alias table.
        let normalized: String = value
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect();
        let tool = match normalized.as_str() {
            "alphafold3" => Self::AlphaFold3,
            "opendde" => Self::OpenDde,
            "boltz" | "boltz2" => Self::Boltz2,
            "chai1" => Self::Chai1,
            "protenix" | "protenixv2" => Self::Protenix,
            "esmfold" | "esmfold2" => Self::EsmFold2,
            "immunebuilder" => Self::ImmuneBuilder,
            "highfold" => Self::HighFold,
            "boltzgen" => Self::BoltzGen,
            "bindcraft" => Self::BindCraft,
            "igblast" => Self::IgBlast,
            "biophi" => Self::BioPhi,
            "antifold" => Self::AntiFold,
            "proteinmpnn" | "abmpnn" => Self::ProteinMpnn,
            "ligandmpnn" => Self::LigandMpnn,
            "proteinmpnnddg" => Self::ProteinMpnnDdg,
            "rfdiffusion" => Self::RfDiffusion,
            "rfantibody" => Self::RfAntibody,
            "germinal" => Self::Germinal,
            "mber" => Self::Mber,
            "igdesign" => Self::IgDesign,
            "thermompnn" => Self::ThermoMpnn,
            "genie3" => Self::Genie3,
            "deepsp" => Self::DeepSp,
            "deepimmuno" => Self::DeepImmuno,
            "tlimmuno" | "tlimmuno2" => Self::TlImmuno2,
            "netsolp" => Self::NetSolP,
            "deepstabp" => Self::DeepStabP,
            "aggrescan3d" => Self::AggreScan3d,
            "dlkcat" => Self::DlkCat,
            "catpred" => Self::CatPred,
            "anarcii" | "antibodyannotator" => Self::Anarcii,
            "placer" => Self::Placer,
            "gromacs" => Self::Gromacs,
            "boltzadme" => Self::BoltzAdme,
            _ => {
                return Err(InstallError::InvalidConfiguration(format!(
                    "unknown tool slug {value:?}"
                )));
            }
        };
        Ok(tool)
    }
}
