//! Much of this module is adapted from `bio_web`; ideally it replaces it.

use std::{fmt, io, path::Path};

use crate::input::InputField;

mod input;
pub mod install;
mod output;
pub mod run;
pub mod status;
mod tool_definitions;
// pub const EXECUTABLES_PATH: &str = "./tool_executables"; // todo: A/R

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LaunchType {
    PythonLib,
    PythonBasedApp,
    CondaBasedApp,
    Executable,
}

impl fmt::Display for LaunchType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::PythonLib => "Python library",
            Self::PythonBasedApp => "Python app (uv)",
            Self::CondaBasedApp => "Python app (conda)",
            Self::Executable => "Executable",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolCategory {
    Cheminformatics,
    StructurePrediction,
    ProteinDesign,
    PeptideBinderDesign,
    MoleculeDynamics,
    QuantumChemistry,
    AntibodyDesign,
    SequencePrediction,
    SequenceAnalysis,
    PropertyPrediction,
    BindingData,
    Placeholder,
}

impl fmt::Display for ToolCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Cheminformatics => "Cheminformatics",
            Self::StructurePrediction => "Structure prediction",
            Self::ProteinDesign => "Protein Design",
            Self::PeptideBinderDesign => "Binder design",
            Self::MoleculeDynamics => "Molecular simulation",
            Self::QuantumChemistry => "Quantum chemistry",
            Self::AntibodyDesign => "Antibody design",
            Self::SequencePrediction => "Sequence prediction",
            Self::SequenceAnalysis => "Sequence analysis",
            Self::PropertyPrediction => "Property prediction",
            Self::BindingData => "Binding data",
            Self::Placeholder => "Uncategorized",
        };
        write!(f, "{}", s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ComputationDeviceConfigured {
    /// CPU only; maybe only supports CPU, or maybe GPU is not configured or available
    Cpu,
    /// Or CPU + GPU
    Gpu,
}

/// These fields can be `None` if not able to be determined, or if not applicable for a given tool.
#[derive(Clone, Debug)]
pub struct InstallationMetadata {
    /// The version of the tool installed
    pub version: String,
    pub computation_device: Option<ComputationDeviceConfigured>,
    /// E.g. CUDA version in use, CPU AVX/SSE instructions available etc or number of cores detected.
    pub computation_details: Option<String>,
    /// Per tool specifics if available.
    pub details: String,
}

/// The status of an individual tool's installation or availability.
/// Each includes descriptive text
#[derive(Clone, Debug)]
pub enum Status {
    Pass(InputField),
    Fault(InstallationMetadata),
    NotFound,
}

/// Used to categorize tools by which OS they support.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OperatingSystem {
    Linux,
    Windows,
    Mac,
}

/// This is coarse, and is loosely correlated to expected runtime.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProcessExpense {
    /// i.e. ms
    Cheap,
    /// i.e. seconds
    Moderate,
    /// I.e. minutes or hours
    Expensive,
}

impl fmt::Display for ProcessExpense {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Cheap => "Cheap (ms)",
            Self::Moderate => "Moderate (s)",
            Self::Expensive => "Expensive (min or hours)",
        };
        write!(f, "{}", s)
    }
}

/// todo: Stub: Augment as required.
#[derive(Debug, Clone, PartialEq)]
pub enum License {
    Mit,
    ApacheV2,
    Other, // todo: Inner string?
}

impl fmt::Display for License {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Mit => "MIT",
            Self::ApacheV2 => "Apache-2.0",
            Self::Other => "Other",
        })
    }
}

impl License {
    pub fn category(self) -> LicenseCategory {
        use License::*;
        match self {
            Mit | ApacheV2 => LicenseCategory::Permissive,
            Other => LicenseCategory::Proprietary,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LicenseData {
    pub license: License,
    /// Only if non-standard, or details like attribution.
    pub details: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LicenseCategory {
    Permissive,
    Copyleft,
    NonCommercial,
    Proprietary,
}

impl fmt::Display for LicenseCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Permissive => "Permissive",
            Self::Copyleft => "Copyleft",
            Self::NonCommercial => "Non-commercial",
            Self::Proprietary => "Proprietary",
        };
        write!(f, "{}", s)
    }
}

/// Alias used by application registries.
pub type ProcessCategory = ToolCategory;

/// Alias used by application registries.
pub type LicenseType = LicenseCategory;

/// Tool-specific descriptive data which does not depend on a UI framework or
/// execution environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Spec {
    pub slug: String,
    pub summary: String,
    pub description: String,
    pub availability: String,
    pub license_details: String,
    pub repo_url: Option<String>,
    pub home_url: Option<String>,
    pub docs_url: Option<String>,
}

impl Spec {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        slug: impl Into<String>,
        summary: impl Into<String>,
        description: impl Into<String>,
        availability: impl Into<String>,
        license_details: impl Into<String>,
        repo_url: Option<String>,
        home_url: Option<String>,
        docs_url: Option<String>,
    ) -> Self {
        Self {
            slug: slug.into(),
            summary: summary.into(),
            description: description.into(),
            availability: availability.into(),
            license_details: license_details.into(),
            repo_url,
            home_url,
            docs_url,
        }
    }

    /// Official links in display order.
    pub fn links(&self) -> Vec<(&'static str, &str)> {
        [
            ("Documentation", self.docs_url.as_deref()),
            ("Home page", self.home_url.as_deref()),
            ("Source code", self.repo_url.as_deref()),
        ]
        .into_iter()
        .filter_map(|(label, url)| url.map(|url| (label, url)))
        .collect()
    }
}

/// Registry data shared by Rust and Python applications.
///
/// UI field descriptors and an adapter implementation are deliberately owned
/// by the consuming application; this type contains the stable, reusable
/// identity and classification of a tool.
#[derive(Clone, Debug, PartialEq)]
pub struct Process {
    pub name: String,
    pub id: u32,
    pub categories: Vec<ProcessCategory>,
    pub launch_type: LaunchType,
    pub license_type: LicenseType,
    pub expense: ProcessExpense,
    pub top_choice: bool,
    pub spec: Spec,
}

impl Process {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: impl Into<String>,
        id: u32,
        categories: Vec<ProcessCategory>,
        launch_type: LaunchType,
        license_type: LicenseType,
        expense: ProcessExpense,
        top_choice: bool,
        spec: Spec,
    ) -> Self {
        Self {
            name: name.into(),
            id,
            categories,
            launch_type,
            license_type,
            expense,
            top_choice,
            spec,
        }
    }

    /// Status probing requires installation layout and is exposed by
    /// install::Installer.
    pub fn status(&self) -> io::Result<Status> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "use Installer::status with a configured installation layout",
        ))
    }

    /// Install this process to the caller's managed tools directory.
    pub fn install(&self, tools_path: &Path) -> io::Result<()> {
        install::Installer::from_environment(tools_path)
            .map_err(io::Error::other)?
            .install(self.tool()?)
            .map_err(io::Error::other)
    }

    /// Remove this process's environment and assets from the managed tools directory.
    pub fn uninstall(&self, tools_path: &Path) -> io::Result<install::UninstallReport> {
        install::Installer::from_environment(tools_path)
            .map_err(io::Error::other)?
            .uninstall(self.tool()?)
            .map_err(io::Error::other)
    }

    fn tool(&self) -> io::Result<install::Tool> {
        self.spec
            .slug
            .parse::<install::Tool>()
            .or_else(|_| self.name.parse::<install::Tool>())
            .map_err(io::Error::other)
    }
}
