use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
};

use bio_tools_rs::{
    LaunchType as RustLaunchType, License as RustLicense, LicenseCategory as RustLicenseCategory,
    Process as RustProcess, ProcessExpense as RustProcessExpense, Spec as RustSpec,
    ToolCategory as RustToolCategory,
    tool_definitions::catalog::{
        self, DataType as RustDataType, PrimaryInput as RustPrimaryInput,
    },
};
use pyo3::{
    PyClass,
    exceptions::PyValueError,
    prelude::*,
    types::{PyDict, PyList},
};

/// Wrap a Rust enum as a frozen Python one.
///
/// A trailing braced block adds methods of the enum's own, which have to be
/// declared here rather than in a second `#[pymethods] impl`: pyo3 allows one
/// such block per class unless its `multiple-pymethods` feature is on.
macro_rules! python_enum {
    (
        $python:ident,
        $python_name:literal,
        $rust:ty,
        {$($variant:ident = $value:expr),+ $(,)?}
        $(, {$($extra:item)*})?
        $(,)?
    ) => {
        #[pyclass(name = $python_name, module = "bio_tools", frozen, skip_from_py_object)]
        pub(crate) struct $python {
            pub(crate) inner: $rust,
            name: &'static str,
            value: u8,
        }

        #[pymethods]
        impl $python {
            $(
                #[classattr]
                #[allow(non_snake_case)]
                fn $variant() -> Self {
                    Self {
                        inner: <$rust>::$variant,
                        name: stringify!($variant),
                        value: $value,
                    }
                }
            )+

            #[getter]
            fn name(&self) -> &'static str {
                self.name
            }

            #[getter]
            fn value(&self) -> u8 {
                self.value
            }

            fn __str__(&self) -> String {
                self.inner.to_string()
            }

            fn __repr__(&self) -> String {
                format!("<{}.{}: {}>", $python_name, self.name, self.value)
            }

            /// Comparison against an unrelated object is `False` rather than a
            /// `TypeError`, matching how Python's own enums behave.
            fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
                other
                    .cast::<Self>()
                    .is_ok_and(|other| self.inner == other.borrow().inner)
            }

            fn __hash__(&self) -> u8 {
                self.value
            }

            $($($extra)*)?
        }

        impl $python {
            /// The Python-facing variant for a Rust enum value, for building one
            /// from data the caller did not construct by hand (e.g. a catalog
            /// entry) rather than from a `#[classattr]`.
            pub(crate) fn from_inner(inner: $rust) -> Self {
                match inner {
                    $(<$rust>::$variant => Self::$variant(),)+
                }
            }
        }
    };
}

python_enum!(
    PyLaunchType,
    "LaunchType",
    RustLaunchType,
    {
        PythonLib = 1,
        PythonBasedApp = 2,
        CondaBasedApp = 3,
        Executable = 4,
    }
);

python_enum!(
    PyToolCategory,
    "ToolCategory",
    RustToolCategory,
    {
        Cheminformatics = 1,
        StructurePrediction = 2,
        ProteinDesign = 3,
        PeptideBinderDesign = 4,
        MoleculeDynamics = 5,
        QuantumChemistry = 6,
        AntibodyDesign = 7,
        SequencePrediction = 8,
        SequenceAnalysis = 9,
        PropertyPrediction = 10,
        BindingData = 11,
        Placeholder = 12,
        BackboneGeneration = 13,
    }
);

python_enum!(
    PyProcessExpense,
    "ProcessExpense",
    RustProcessExpense,
    {
        Cheap = 1,
        Moderate = 2,
        Expensive = 3,
    }
);

python_enum!(
    PyLicenseCategory,
    "LicenseCategory",
    RustLicenseCategory,
    {
        Permissive = 1,
        Copyleft = 2,
        NonCommercial = 3,
        Proprietary = 4,
    }
);

python_enum!(
    PyLicense,
    "License",
    RustLicense,
    {
        Mit = 1,
        ApacheV2 = 2,
        Bsd3Clause = 3,
        Lgpl21OrLater = 4,
        PublicDomain = 5,
        Other = 6,
    }
);

python_enum!(
    PyDataType,
    "DataType",
    RustDataType,
    {
        MmCif = 1,
        Pdb = 2,
        AaSequence = 3,
        DnaSequence = 4,
        RnaSequence = 5,
        Csv = 6,
    },
    {
        /// "Structure", "Sequence" or "Table": the family, which is what
        /// finding the file a run left comes down to.
        #[getter]
        fn category(&self) -> String {
            self.inner.category().to_string()
        }

        /// A word for a job row, where there is room for one: "Structure", "Seq".
        #[getter]
        fn label(&self) -> &'static str {
            self.inner.category().label()
        }

        /// Every suffix this family is written with, lower-case and before any `.gz`.
        #[getter]
        fn suffixes(&self) -> Vec<&'static str> {
            self.inner.category().suffixes().to_vec()
        }

        /// Whether a file of this type can be handed to an input wanting `wanted`.
        fn feeds(&self, py: Python<'_>, wanted: Py<Self>) -> bool {
            self.inner.feeds(wanted.borrow(py).inner)
        }
    }
);

/// The one file a tool is chiefly given: the field it goes in, and what that
/// field accepts there.
#[pyclass(
    name = "PrimaryInput",
    module = "bio_tools",
    frozen,
    skip_from_py_object
)]
pub(crate) struct PyPrimaryInput {
    inner: RustPrimaryInput,
}

#[pymethods]
impl PyPrimaryInput {
    /// The field, by the `name` it carries in this tool's field descriptors.
    #[getter]
    fn field(&self) -> &'static str {
        self.inner.field
    }

    #[getter]
    fn accepts(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let values = PyList::empty(py);
        for data_type in self.inner.accepts {
            values.append(Py::new(py, PyDataType::from_inner(*data_type))?)?;
        }
        Ok(values.into_any().unbind())
    }

    /// "file" for the bytes exactly as the producing tool wrote them,
    /// "residues" for a field that wants bare sequence with FASTA headers
    /// dropped, "document" for one holding a whole input document that a
    /// chosen sequence joins as one more chain.
    #[getter]
    fn form(&self) -> &'static str {
        self.inner.form.kind()
    }

    /// Which document dialect that field holds, for a "document" form.
    #[getter]
    fn dialect(&self) -> Option<&'static str> {
        self.inner.form.dialect()
    }

    /// Whether a file of `produced` can be dropped into this field.
    fn accepts_type(&self, py: Python<'_>, produced: Py<PyDataType>) -> bool {
        self.inner.accepts(produced.borrow(py).inner)
    }

    fn __repr__(&self) -> String {
        format!(
            "PrimaryInput(field={:?}, form={:?})",
            self.inner.field,
            self.form()
        )
    }
}

/// Shared tool description plus application-owned field descriptors.
#[pyclass(name = "Spec", module = "bio_tools", frozen, skip_from_py_object)]
pub(crate) struct PySpec {
    inner: RustSpec,
    fields: Py<PyAny>,
    refresh_fields: Option<Py<PyAny>>,
    tasks: Py<PyAny>,
}

#[pymethods]
impl PySpec {
    #[new]
    #[pyo3(signature = (
        *,
        slug,
        summary,
        description,
        availability,
        license_details,
        fields,
        repo_url=None,
        home_url=None,
        docs_url=None,
        input_params_url=None,
        examples_url=None,
        paper_url=None,
        license=None,
        license_url=None,
        tested=false,
        refresh_fields=None,
        tasks=None
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        slug: String,
        summary: String,
        description: String,
        availability: String,
        license_details: String,
        fields: Py<PyAny>,
        repo_url: Option<String>,
        home_url: Option<String>,
        docs_url: Option<String>,
        input_params_url: Option<String>,
        examples_url: Option<String>,
        paper_url: Option<String>,
        license: Option<Py<PyLicense>>,
        license_url: Option<String>,
        tested: bool,
        refresh_fields: Option<Py<PyAny>>,
        tasks: Option<Py<PyAny>>,
    ) -> Self {
        let license = license
            .map(|license| license.borrow(py).inner)
            .unwrap_or(RustLicense::Other);
        let inner = RustSpec::new(
            slug,
            summary,
            description,
            availability,
            license_details,
            repo_url,
            home_url,
            docs_url,
            input_params_url,
            examples_url,
            paper_url,
            license,
            license_url,
            tested,
        );
        Self {
            inner,
            fields,
            refresh_fields,
            tasks: tasks.unwrap_or_else(|| PyList::empty(py).into_any().unbind()),
        }
    }

    #[getter]
    fn slug(&self) -> &str {
        &self.inner.slug
    }

    #[getter]
    fn summary(&self) -> &str {
        &self.inner.data.summary
    }

    #[getter]
    fn description(&self) -> &str {
        &self.inner.data.description
    }

    #[getter]
    fn availability(&self) -> &str {
        &self.inner.data.availability
    }

    #[getter]
    fn license_details(&self) -> &str {
        &self.inner.data.license_details
    }

    #[getter]
    fn repo_url(&self) -> Option<&str> {
        self.inner.data.repo_url.as_deref()
    }

    #[getter]
    fn home_url(&self) -> Option<&str> {
        self.inner.data.home_url.as_deref()
    }

    #[getter]
    fn docs_url(&self) -> Option<&str> {
        self.inner.data.docs_url.as_deref()
    }

    #[getter]
    fn input_params_url(&self) -> Option<&str> {
        self.inner.data.input_params_url.as_deref()
    }

    #[getter]
    fn examples_url(&self) -> Option<&str> {
        self.inner.data.examples_url.as_deref()
    }

    #[getter]
    fn paper_url(&self) -> Option<&str> {
        self.inner.data.paper_url.as_deref()
    }

    #[getter]
    fn license(&self, py: Python<'_>) -> PyResult<Py<PyLicense>> {
        Py::new(py, PyLicense::from_inner(self.inner.data.license))
    }

    #[getter]
    fn license_url(&self) -> Option<&str> {
        self.inner.data.license_url.as_deref()
    }

    #[getter]
    fn tested(&self) -> bool {
        self.inner.data.tested
    }

    #[getter]
    fn fields(&self, py: Python<'_>) -> Py<PyAny> {
        self.fields.clone_ref(py)
    }

    #[getter]
    fn refresh_fields(&self, py: Python<'_>) -> Option<Py<PyAny>> {
        self.refresh_fields
            .as_ref()
            .map(|callback| callback.clone_ref(py))
    }

    #[getter]
    fn tasks(&self, py: Python<'_>) -> Py<PyAny> {
        self.tasks.clone_ref(py)
    }

    fn active_fields(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match &self.refresh_fields {
            Some(callback) => callback.call0(py),
            None => Ok(self.fields.clone_ref(py)),
        }
    }

    fn links(&self) -> Vec<BTreeMap<&'static str, String>> {
        self.inner
            .links()
            .into_iter()
            .map(|(label, url)| {
                BTreeMap::from([("label", label.to_owned()), ("url", url.to_owned())])
            })
            .collect()
    }

    fn serialize(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.serialize_dict(py)
    }

    fn __repr__(&self) -> String {
        format!(
            "Spec(slug={:?}, summary={:?})",
            self.inner.slug, self.inner.data.summary
        )
    }
}

impl PySpec {
    fn serialize_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let data = PyDict::new(py);
        data.set_item("slug", &self.inner.slug)?;
        data.set_item("summary", &self.inner.data.summary)?;
        data.set_item("description", &self.inner.data.description)?;
        data.set_item("availability", &self.inner.data.availability)?;
        data.set_item("license_details", &self.inner.data.license_details)?;
        data.set_item("repo_url", &self.inner.data.repo_url)?;
        data.set_item("home_url", &self.inner.data.home_url)?;
        data.set_item("docs_url", &self.inner.data.docs_url)?;
        data.set_item("input_params_url", &self.inner.data.input_params_url)?;
        data.set_item("examples_url", &self.inner.data.examples_url)?;
        data.set_item("paper_url", &self.inner.data.paper_url)?;
        data.set_item("license", self.inner.data.license.to_string())?;
        data.set_item("license_url", &self.inner.data.license_url)?;
        data.set_item("tested", &self.inner.data.tested)?;

        // What this tool hands on, and what it can be handed: the pair a
        // consuming UI needs to offer one run's output as the next run's input.
        if let Some(entry) = catalog::by_slug(&self.inner.slug) {
            data.set_item(
                "primary_output",
                entry.primary_output.map(RustDataType::as_str),
            )?;
            data.set_item("primary_inputs", serialize_primary_inputs(py, entry)?)?;
        }

        let fields = self.active_fields(py)?;
        data.set_item("fields", serialize_dataclasses(py, &fields)?)?;
        data.set_item("tasks", serialize_dataclasses(py, &self.tasks)?)?;
        data.set_item("presets", catalog_presets(py, &self.inner.slug)?)?;

        if let Some(source) = bio_tools_rs::tool_definitions::fields::by_slug(&self.inner.slug) {
            let contract = py.import("json")?.call_method1("loads", (source,))?;
            data.set_item(
                "input_modes",
                contract.call_method1("get", ("input_modes",))?,
            )?;
            data.set_item(
                "task_group",
                contract.call_method1("get", ("task_group", ""))?,
            )?;
            data.set_item(
                "field_groups",
                contract.call_method1("get", ("field_groups", PyList::empty(py)))?,
            )?;
        }

        data.set_item("links", self.links())?;
        Ok(data.unbind())
    }
}

/// Shared registry entry. The adapter module remains an opaque application
/// value while identity and classification are represented by Rust types.
#[pyclass(name = "Process", module = "bio_tools", frozen, skip_from_py_object)]
pub(crate) struct PyProcess {
    inner: RustProcess,
    categories: Vec<Py<PyToolCategory>>,
    launch_type: Py<PyLaunchType>,
    license_type: Py<PyLicenseCategory>,
    expense: Py<PyProcessExpense>,
    module: Py<PyAny>,
    spec: Py<PySpec>,
}

#[pymethods]
impl PyProcess {
    #[new]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        name: String,
        id: u32,
        categories: Vec<Py<PyToolCategory>>,
        launch_type: Py<PyLaunchType>,
        license_type: Py<PyLicenseCategory>,
        expense: Py<PyProcessExpense>,
        module: Py<PyAny>,
        top_choice: bool,
        spec: Py<PySpec>,
    ) -> Self {
        let inner = RustProcess::new(
            name,
            id,
            categories
                .iter()
                .map(|category| category.borrow(py).inner)
                .collect(),
            launch_type.borrow(py).inner,
            license_type.borrow(py).inner,
            expense.borrow(py).inner,
            top_choice,
            spec.borrow(py).inner.clone(),
        );
        Self {
            inner,
            categories,
            launch_type,
            license_type,
            expense,
            module,
            spec,
        }
    }

    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn id(&self) -> u32 {
        self.inner.id
    }

    #[getter]
    fn categories(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        list_from_objects(py, &self.categories)
    }

    #[getter]
    fn launch_type(&self, py: Python<'_>) -> Py<PyLaunchType> {
        self.launch_type.clone_ref(py)
    }

    #[getter]
    fn license_type(&self, py: Python<'_>) -> Py<PyLicenseCategory> {
        self.license_type.clone_ref(py)
    }

    #[getter]
    fn expense(&self, py: Python<'_>) -> Py<PyProcessExpense> {
        self.expense.clone_ref(py)
    }

    #[getter]
    fn module(&self, py: Python<'_>) -> Py<PyAny> {
        self.module.clone_ref(py)
    }

    #[getter]
    fn top_choice(&self) -> bool {
        self.inner.top_choice
    }

    #[getter]
    fn spec(&self, py: Python<'_>) -> Py<PySpec> {
        self.spec.clone_ref(py)
    }

    /// The kind of file this tool's headline output is, or None.
    #[getter]
    fn primary_output(&self, py: Python<'_>) -> PyResult<Option<Py<PyDataType>>> {
        catalog_primary_output(py, &self.inner.spec.slug)
    }

    /// Where a previous run's output can be dropped into this tool's form.
    #[getter]
    fn primary_inputs(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        catalog_primary_inputs(py, &self.inner.spec.slug)
    }

    fn serialize(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let data = self.spec.borrow(py).serialize_dict(py)?;
        let data = data.bind(py);
        data.set_item("name", &self.inner.name)?;
        let categories = self
            .categories
            .iter()
            .map(|category| category.borrow(py).inner.to_string())
            .collect::<Vec<_>>();
        data.set_item("categories", categories)?;
        data.set_item("launch_type", self.inner.launch_type.to_string())?;
        data.set_item("license_type", self.inner.license_type.to_string())?;
        data.set_item("expense", self.inner.expense.to_string())?;
        Ok(data.clone().unbind())
    }

    fn __repr__(&self) -> String {
        format!("Process(name={:?}, id={})", self.inner.name, self.inner.id)
    }
}

fn form_catalog_entry<'py>(py: Python<'py>, slug: &str) -> PyResult<Bound<'py, PyDict>> {
    let source = bio_tools_rs::tool_definitions::fields::by_slug(slug).ok_or_else(|| {
        PyValueError::new_err(format!("no bio_tools field catalog for slug {slug:?}"))
    })?;
    let entry = py.import("json")?.call_method1("loads", (source,))?;
    Ok(entry.cast::<PyDict>().map_err(PyErr::from)?.clone())
}

/// List named example inputs. Each descriptor includes editable `values` and
/// its source URL. Bundled file values use `bio-tools://<slug>/<asset name>`.
#[pyfunction]
fn catalog_presets(py: Python<'_>, slug: &str) -> PyResult<Py<PyAny>> {
    let source = bio_tools_rs::tool_definitions::presets::by_slug(slug).unwrap_or("[]");
    Ok(py
        .import("json")?
        .call_method1("loads", (source,))?
        .unbind())
}

/// Return a complete preset payload, including the tool's ordinary defaults.
/// Explicit overrides win, including empty file values. Pass workdir to copy all
/// bundled assets there and replace references with absolute paths, including
/// references inside JSON input documents. Omit it for portable form values.
#[pyfunction]
#[pyo3(signature = (slug, preset_id, *, overrides=None, workdir=None))]
fn catalog_preset(
    py: Python<'_>,
    slug: &str,
    preset_id: &str,
    overrides: Option<Bound<'_, PyDict>>,
    workdir: Option<PathBuf>,
) -> PyResult<Py<PyDict>> {
    let json = py.import("json")?;
    let overrides = match overrides {
        Some(values) => {
            serde_json::from_str(&json.call_method1("dumps", (values,))?.extract::<String>()?)
                .map_err(|error| PyValueError::new_err(error.to_string()))?
        }
        None => serde_json::Map::new(),
    };
    let mut payload = bio_tools_rs::tool_definitions::presets::payload(slug, preset_id, &overrides)
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
    if let Some(directory) = workdir {
        payload = bio_tools_rs::tool_definitions::presets::materialize(slug, &payload, &directory)?;
    }
    Ok(json
        .call_method1("loads", (payload.to_string(),))?
        .cast::<PyDict>()?
        .clone()
        .unbind())
}

/// The kind of file this tool's headline output is, or None where it has no
/// single one. Keyed by slug so a caller holding only a finished run's tool
/// name can ask; see also `Process.primary_output`.
#[pyfunction]
fn catalog_primary_output(py: Python<'_>, slug: &str) -> PyResult<Option<Py<PyDataType>>> {
    let Some(entry) = catalog::by_slug(slug) else {
        return Ok(None);
    };
    entry
        .primary_output
        .map(|data_type| Py::new(py, PyDataType::from_inner(data_type)))
        .transpose()
}

/// Where a previous run's output can be dropped into this tool's form: one
/// entry per field that takes one, which for a tool offering a choice of input
/// modes is one per mode. Empty for a tool nothing can be fed to.
#[pyfunction]
fn catalog_primary_inputs(py: Python<'_>, slug: &str) -> PyResult<Py<PyAny>> {
    let values = PyList::empty(py);
    if let Some(entry) = catalog::by_slug(slug) {
        for inner in entry.primary_inputs {
            values.append(Py::new(py, PyPrimaryInput { inner: *inner })?)?;
        }
    }
    Ok(values.into_any().unbind())
}

fn serialize_primary_inputs<'py>(
    py: Python<'py>,
    entry: &catalog::CatalogEntry,
) -> PyResult<Bound<'py, PyList>> {
    let values = PyList::empty(py);
    for input in entry.primary_inputs {
        let data = PyDict::new(py);
        data.set_item("field", input.field)?;
        data.set_item(
            "accepts",
            input
                .accepts
                .iter()
                .map(|data_type| data_type.as_str())
                .collect::<Vec<_>>(),
        )?;
        data.set_item("form", input.form.kind())?;
        data.set_item("dialect", input.form.dialect())?;
        values.append(data)?;
    }
    Ok(values)
}

/// Resolve a bundled asset reference, or return ordinary uploaded/pasted text unchanged.
#[pyfunction]
fn catalog_input_text(slug: &str, value: &str) -> PyResult<String> {
    bio_tools_rs::tool_definitions::presets::input_text(slug, value)
        .map(str::to_owned)
        .map_err(|error| PyValueError::new_err(error.to_string()))
}

/// Read an embedded example asset without downloading it or exposing local paths.
#[pyfunction]
fn catalog_asset(slug: &str, name: &str) -> PyResult<&'static str> {
    bio_tools_rs::tool_definitions::presets::asset(slug, name).ok_or_else(|| {
        PyValueError::new_err(format!("unknown bundled asset {name:?} for {slug:?}"))
    })
}
/// Materialize catalog-owned form descriptors using a consumer's Field and
/// Option classes. Dynamic select options are supplied by the consumer because
/// they reflect resources installed on that particular host.
#[pyfunction]
#[pyo3(signature = (slug, *, field_type, option_type, dynamic_options=None))]
fn catalog_fields(
    py: Python<'_>,
    slug: &str,
    field_type: Py<PyAny>,
    option_type: Py<PyAny>,
    dynamic_options: Option<HashMap<String, Vec<(String, String)>>>,
) -> PyResult<Py<PyAny>> {
    let entry = form_catalog_entry(py, slug)?;

    let definitions: Bound<'_, PyList> = entry
        .get_item("fields")?
        .ok_or_else(|| {
            PyValueError::new_err(format!("invalid bio_tools field catalog for slug {slug:?}"))
        })?
        .extract()?;
    let values = PyList::empty(py);
    for descriptor in definitions.iter() {
        let descriptor = descriptor.cast::<PyDict>()?;
        let get = |key| {
            descriptor.get_item(key)?.ok_or_else(|| {
                PyValueError::new_err(format!(
                    "invalid field descriptor for {slug:?}: missing {key}"
                ))
            })
        };
        let name: String = get("name")?.extract()?;
        let label: String = get("label")?.extract()?;
        let kind: String = get("kind")?.extract()?;
        let kwargs = PyDict::new(py);
        for key in [
            "default",
            "required",
            "help",
            "rows",
            "minimum",
            "maximum",
            "step",
            "maxlength",
            "accept",
            "task",
        ] {
            kwargs.set_item(key, get(key)?)?;
        }
        for key in ["group", "input_modes", "help_note", "molecule_features"] {
            if let Some(value) = descriptor.get_item(key)? {
                kwargs.set_item(key, value)?;
            }
        }
        let options = PyList::empty(py);
        if let Some(items) = dynamic_options.as_ref().and_then(|items| items.get(&name)) {
            for (value, label) in items {
                options.append(option_type.call1(py, (value, label))?)?;
            }
            if matches!(name.as_str(), "germline_db_v" | "germline_db_j") {
                if let Some((value, _)) = items.first() {
                    kwargs.set_item("default", value)?;
                }
            }
        } else {
            let original: Bound<'_, PyList> = get("options")?.extract()?;
            for option in original.iter() {
                let option = option.cast::<PyDict>()?;
                let value = option.get_item("value")?.expect("serialized option value");
                let label = option.get_item("label")?.expect("serialized option label");
                options.append(option_type.call1(py, (value, label))?)?;
            }
        }
        kwargs.set_item("options", options)?;
        values.append(field_type.call(py, (name, label, kind), Some(&kwargs))?)?;
    }
    Ok(values.into_any().unbind())
}
/// Materialize catalog-owned task selectors using a consumer's Option class.
#[pyfunction]
#[pyo3(signature = (slug, *, option_type))]
fn catalog_tasks(py: Python<'_>, slug: &str, option_type: Py<PyAny>) -> PyResult<Py<PyAny>> {
    let entry = form_catalog_entry(py, slug)?;

    let tasks: Bound<'_, PyList> = entry
        .get_item("tasks")?
        .ok_or_else(|| {
            PyValueError::new_err(format!("invalid bio_tools field catalog for slug {slug:?}"))
        })?
        .extract()?;
    let values = PyList::empty(py);
    for task in tasks.iter() {
        let task = task.cast::<PyDict>()?;
        let value = task.get_item("value")?.expect("serialized task value");
        let label = task.get_item("label")?.expect("serialized task label");
        values.append(option_type.call1(py, (value, label))?)?;
    }
    Ok(values.into_any().unbind())
}
/// Build a [`PySpec`] from bio_tools' central catalog by slug, so a caller
/// supplies only what is genuinely its own: UI field descriptors.
#[pyfunction]
#[pyo3(signature = (slug, *, fields, refresh_fields=None, tasks=None))]
fn catalog_spec(
    py: Python<'_>,
    slug: &str,
    fields: Py<PyAny>,
    refresh_fields: Option<Py<PyAny>>,
    tasks: Option<Py<PyAny>>,
) -> PyResult<PySpec> {
    let entry = catalog::by_slug(slug).ok_or_else(|| {
        PyValueError::new_err(format!("no bio_tools catalog entry for slug {slug:?}"))
    })?;
    Ok(PySpec {
        inner: entry.to_spec(),
        fields,
        refresh_fields,
        tasks: tasks.unwrap_or_else(|| PyList::empty(py).into_any().unbind()),
    })
}

/// Build a [`PyProcess`] from bio_tools' central catalog, keyed by the slug
/// already on `module.SPEC` (itself built by [`catalog_spec`]). A caller
/// supplies only what is genuinely its own: a numeric id for its own storage
/// and the adapter module.
#[pyfunction]
fn catalog_process(py: Python<'_>, id: u32, module: Py<PyAny>) -> PyResult<PyProcess> {
    let spec: Py<PySpec> = module.bind(py).getattr("SPEC")?.extract()?;
    let slug = spec.borrow(py).inner.slug.clone();
    let entry = catalog::by_slug(&slug).ok_or_else(|| {
        PyValueError::new_err(format!("no bio_tools catalog entry for slug {slug:?}"))
    })?;

    let categories = entry
        .categories
        .iter()
        .map(|category| Py::new(py, PyToolCategory::from_inner(*category)))
        .collect::<PyResult<Vec<_>>>()?;
    let launch_type = Py::new(py, PyLaunchType::from_inner(entry.launch_type))?;
    let license_type = Py::new(py, PyLicenseCategory::from_inner(entry.license_type))?;
    let expense = Py::new(py, PyProcessExpense::from_inner(entry.expense))?;

    Ok(PyProcess::new(
        py,
        entry.name().to_owned(),
        id,
        categories,
        launch_type,
        license_type,
        expense,
        module,
        entry.top_choice,
        spec,
    ))
}

fn serialize_dataclasses(py: Python<'_>, values: &Py<PyAny>) -> PyResult<Py<PyAny>> {
    let asdict = py.import("dataclasses")?.getattr("asdict")?;
    let serialized = PyList::empty(py);
    for value in values.bind(py).try_iter()? {
        serialized.append(asdict.call1((value?,))?)?;
    }
    Ok(serialized.into_any().unbind())
}

fn list_from_objects<T: PyClass>(py: Python<'_>, values: &[Py<T>]) -> PyResult<Py<PyAny>> {
    let list = PyList::empty(py);
    for value in values {
        list.append(value.bind(py))?;
    }
    Ok(list.into_any().unbind())
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyLaunchType>()?;
    module.add_class::<PyToolCategory>()?;
    module.add_class::<PyProcessExpense>()?;
    module.add_class::<PyLicenseCategory>()?;
    module.add_class::<PyLicense>()?;
    module.add_class::<PyDataType>()?;
    module.add_class::<PyPrimaryInput>()?;
    module.add_class::<PySpec>()?;
    module.add_class::<PyProcess>()?;
    module.add_function(wrap_pyfunction!(catalog_fields, module)?)?;
    module.add_function(wrap_pyfunction!(catalog_tasks, module)?)?;
    module.add_function(wrap_pyfunction!(catalog_presets, module)?)?;
    module.add_function(wrap_pyfunction!(catalog_preset, module)?)?;
    module.add_function(wrap_pyfunction!(catalog_input_text, module)?)?;
    module.add_function(wrap_pyfunction!(catalog_primary_output, module)?)?;
    module.add_function(wrap_pyfunction!(catalog_primary_inputs, module)?)?;
    module.add_function(wrap_pyfunction!(catalog_asset, module)?)?;
    module.add_function(wrap_pyfunction!(catalog_spec, module)?)?;
    module.add_function(wrap_pyfunction!(catalog_process, module)?)?;
    Ok(())
}
