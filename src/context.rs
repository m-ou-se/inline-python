use crate::PythonBlock;
use crate::run::run_python_code;
use pyo3::{
    FromPyObject, IntoPyObject, Py, PyResult, Python,
    prelude::*,
    types::{PyCFunction, PyDict},
};

/// An execution context for Python code.
///
/// This can be used to keep all global variables and imports intact between macro invocations:
///
/// ```
/// # use inline_python::{Context, python};
/// let c = Context::new();
///
/// c.run(python! {
///     foo = 5
/// });
///
/// c.run(python! {
///     assert foo == 5
/// });
/// ```
///
/// You may also use it to inspect global variables after the execution of the Python code,
/// or set global variables before running:
///
/// ```
/// # use inline_python::{Context, python};
/// let c = Context::new();
///
/// c.set("x", 13);
///
/// c.run(python! {
///     foo = x + 2
/// });
///
/// assert_eq!(c.get::<i32>("foo"), 15);
/// ```
pub struct Context {
    pub(crate) globals: Py<PyDict>,
}

impl Context {
    /// Create a new context for running Python code.
    ///
    /// This function panics if it fails to create the context.
    #[allow(clippy::new_without_default)]
    #[track_caller]
    pub fn new() -> Self {
        Python::with_gil(Self::new_with_gil)
    }

    #[track_caller]
    pub(crate) fn new_with_gil(py: Python) -> Self {
        match Self::try_new(py) {
            Ok(x) => x,
            Err(err) => panic!("{}", panic_string(py, &err)),
        }
    }

    fn try_new(py: Python) -> PyResult<Self> {
        Ok(Self {
            globals: py.import("__main__")?.dict().copy()?.into(),
        })
    }

    /// Get the globals as dictionary.
    pub fn globals(&self) -> &Py<PyDict> {
        &self.globals
    }

    /// Retrieve a global variable from the context.
    ///
    /// This function panics if the variable doesn't exist, or the conversion fails.
    pub fn get<T: for<'p> FromPyObject<'p>>(&self, name: &str) -> T {
        Python::with_gil(|py| match self.globals.bind(py).get_item(name) {
            Err(_) | Ok(None) => {
                panic!("Python context does not contain a variable named `{name}`",)
            }
            Ok(Some(value)) => match FromPyObject::extract_bound(&value) {
                Ok(value) => value,
                Err(e) => panic!(
                    "Unable to convert `{name}` to `{ty}`: {e}",
                    ty = std::any::type_name::<T>(),
                ),
            },
        })
    }

    /// Set a global variable in the context.
    ///
    /// This function panics if the conversion fails.
    pub fn set<T: for<'p> IntoPyObject<'p>>(&self, name: &str, value: T) {
        Python::with_gil(|py| {
            if let Err(e) = self.globals().bind(py).set_item(name, value) {
                panic!(
                    "Unable to set `{name}` from a `{ty}`: {e}",
                    ty = std::any::type_name::<T>(),
                );
            }
        })
    }

    /// Add a wrapped `#[pyfunction]` or `#[pymodule]` using its own `__name__`.
    ///
    /// Use this with `pyo3::wrap_pyfunction` or `pyo3::wrap_pymodule`.
    ///
    /// ```ignore
    /// # use inline_python::{Context, python};
    /// use pyo3::{prelude::*, wrap_pyfunction};
    ///
    /// #[pyfunction]
    /// fn get_five() -> i32 {
    ///     5
    /// }
    ///
    /// fn main() {
    ///     let c = Context::new();
    ///
    ///     c.add_wrapped(wrap_pyfunction!(get_five));
    ///
    ///     c.run(python! {
    ///         assert get_five() == 5
    ///     });
    /// }
    /// ```
    pub fn add_wrapped(&self, wrapper: &impl Fn(Python) -> PyResult<Bound<'_, PyCFunction>>) {
        Python::with_gil(|py| {
            let obj = wrapper(py).unwrap();
            let name = obj
                .getattr("__name__")
                .expect("wrapped item should have a __name__");
            if let Err(err) = self.globals().bind(py).set_item(name, obj) {
                panic!("{}", panic_string(py, &err));
            }
        })
    }

    /// Run Python code using this context.
    ///
    /// This function should be called using the `python!{}` macro:
    ///
    /// ```
    /// # use inline_python::{Context, python};
    /// let c = Context::new();
    ///
    /// c.run(python!{
    ///     print("Hello World")
    /// });
    /// ```
    ///
    /// This function panics if the Python code fails.
    pub fn run(
        &self,
        #[cfg(not(doc))] code: PythonBlock<impl FnOnce(&Bound<PyDict>)>,
        #[cfg(doc)] code: PythonBlock, // Just show 'PythonBlock' in the docs.
    ) {
        Python::with_gil(|py| self.run_with_gil(py, code));
    }

    #[cfg(not(doc))]
    pub(crate) fn run_with_gil<F: FnOnce(&Bound<PyDict>)>(
        &self,
        py: Python<'_>,
        block: PythonBlock<F>,
    ) {
        (block.set_vars)(self.globals().bind(py));
        if let Err(err) = run_python_code(py, self, block.bytecode) {
            (block.panic)(panic_string(py, &err));
        }
    }
}

fn panic_string(py: Python, err: &PyErr) -> String {
    match py_err_to_string(py, &err) {
        Ok(msg) => msg,
        Err(_) => err.to_string(),
    }
}

/// Print the error while capturing stderr into a String.
fn py_err_to_string(py: Python, err: &PyErr) -> Result<String, PyErr> {
    let sys = py.import("sys")?;
    let stderr = py.import("io")?.getattr("StringIO")?.call0()?;
    let original_stderr = sys.dict().get_item("stderr")?;
    sys.dict().set_item("stderr", &stderr)?;
    err.print(py);
    sys.dict().set_item("stderr", original_stderr)?;
    stderr.call_method0("getvalue")?.extract()
}
