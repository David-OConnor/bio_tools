use std::collections::{BTreeMap, HashMap};

use bio_tools_rs::{
    LaunchType as RustLaunchType, License as RustLicense,
    LicenseCategory as RustLicenseCategory, Process as RustProcess,
    ProcessExpense as RustProcessExpense, Spec as RustSpec, ToolCategory as RustToolCategory,
    tool_definitions::catalog,
};
use pyo3::{
    PyClass,
    exceptions::PyValueError,
    prelude::*,
    types::{PyDict, PyList},
};

macro_rules! python_enum {
    ($python:ident, $python_name:literal, $rust:ty, {$($variant:ident = $value:expr),+ $(,)?}) => {
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

            fn __eq__(&self, other: &Self) -> bool {
                self.inner == other.inner
            }

            fn __hash__(&self) -> u8 {
                self.value
            }
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
        paper_url=None,
        license=None,
        license_url=None,
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
        paper_url: Option<String>,
        license: Option<Py<PyLicense>>,
        license_url: Option<String>,
        refresh_fields: Option<Py<PyAny>>,
        tasks: Option<Py<PyAny>>,
    ) -> Self {
        let license = license
            .map(|license| license.borrow(py).inner)
            .unwrap_or(RustLicense::Other);
        Self {
            inner: RustSpec::new(
                slug,
                summary,
                description,
                availability,
                license_details,
                repo_url,
                home_url,
                docs_url,
                paper_url,
                license,
                license_url,
            ),
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
        data.set_item("paper_url", &self.inner.data.paper_url)?;
        data.set_item("license", self.inner.data.license.to_string())?;
        data.set_item("license_url", &self.inner.data.license_url)?;
        let fields = self.active_fields(py)?;
        data.set_item("fields", serialize_dataclasses(py, &fields)?)?;
        data.set_item("tasks", serialize_dataclasses(py, &self.tasks)?)?;
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
    let source = catalog::fields::by_slug(slug).ok_or_else(|| {
        PyValueError::new_err(format!("no bio_tools field catalog for slug {slug:?}"))
    })?;
    let entry = py.import("json")?.call_method1("loads", (source,))?;
    Ok(entry.cast::<PyDict>().map_err(PyErr::from)?.clone())
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
    module.add_class::<PySpec>()?;
    module.add_class::<PyProcess>()?;
    module.add_function(wrap_pyfunction!(catalog_fields, module)?)?;
    module.add_function(wrap_pyfunction!(catalog_tasks, module)?)?;
    module.add_function(wrap_pyfunction!(catalog_spec, module)?)?;
    module.add_function(wrap_pyfunction!(catalog_process, module)?)?;
    Ok(())
}
