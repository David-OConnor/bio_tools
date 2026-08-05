//! Much of this module is adapted from `bio_web`; ideally it replaces it.

use std::{fmt, io, path::Path};

use crate::input::InputField;

mod input;
pub mod install;
mod output;
mod run;
mod tool_definitions;
mod status;
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

/// This is the top-level definition of an individual tool. It is broad, and applies
/// to all tools this library manages.
/// todo: Consider renaming as `Tool`.
#[derive(Clone, Debug)]
pub struct Process {
    pub name: String,
    pub id: u32,
    pub categories: Vec<ToolCategory>,
    pub launch_type: LaunchType,
    pub license: LicenseData,
    pub operating_systems: Vec<OperatingSystem>,
    pub gpu_desired: bool,
    pub expense: ProcessExpense,
    pub top_choice: bool,
    pub input_fields: Vec<InputField>,
    /// E.g. the executable name, or python process exposed on the PATH; usd to launch
    /// with via CLI.
    pub executable_name: String,
    pub summary: String,
    pub description: String,
    pub availability: String,
    pub license_details: String,
    pub repo_url: Option<String>,
    pub home_url: Option<String>,
    pub docs_url: Option<String>,
    // pub refresh_fields: Option<fn() -> Vec<Field>>,
    // pub tasks: Vec<TaskOption>,
}

impl Process {
    /// Status probing is tool-specific and is not yet described by [`Process`].
    pub fn status(&self) -> io::Result<Status> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "this process does not define a status probe",
        ))
    }

    /// Install this process to the managed applications/tools directory.
    /// The caller sets the application path; this
    /// is the outer path, e.g. likely the same for all that caller's tools.
    pub fn install(&self, tools_path: &Path) -> io::Result<()> {
        let tool = self
            .executable_name
            .parse::<install::Tool>()
            .or_else(|_| self.name.parse::<install::Tool>())
            .map_err(io::Error::other)?;
        install::Installer::from_environment(tools_path)
            .map_err(io::Error::other)?
            .install(tool)
            .map_err(io::Error::other)
    }

    /// Uninstall this process from the managed applications/tools directory. The caller sets the application path; this
    /// is the outer path, e.g. likely the same for all that caller's tools.
    pub fn uninstall(&self, _tools_path: &Path) -> io::Result<()> {
        if let Status::NotFound = self.status()? {
            return Err(io::Error::other("Process not found"));
        }

        Ok(())
    }
}
