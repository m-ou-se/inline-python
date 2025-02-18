use crate::run::run_python_code;
use crate::PythonBlock;
use pyo3::{
	prelude::*,
	types::{PyCFunction, PyDict},
	FromPyObject, IntoPyObject, Py, PyResult, Python,
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
///   foo = 5
/// });
///
/// c.run(python! {
///   assert foo == 5
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
///   foo = x + 2
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
	pub fn new() -> Self {
		Python::with_gil(Self::new_with_gil)
	}

	pub(crate) fn new_with_gil(py: Python) -> Self {
		match Self::try_new(py) {
			Ok(x) => x,
			Err(error) => {
				error.print(py);
				panic!("failed to create Python context");
			}
		}
	}

	fn try_new(py: Python) -> PyResult<Self> {
		Ok(Self {
			globals: py.import("__main__")?.dict().copy()?.into(),
		})
	}

	/// Get the globals as dictionary.
	pub fn globals<'p>(&self) -> &Py<PyDict> {
		&self.globals
	}

	/// Retrieve a global variable from the context.
	///
	/// This function panics if the variable doesn't exist, or the conversion fails.
	pub fn get<T: for<'p> FromPyObject<'p>>(&self, name: &str) -> T {
		Python::with_gil(|py| match self.globals.bind(py).get_item(name) {
			Err(_) | Ok(None) => panic!("Python context does not contain a variable named `{}`", name),
			Ok(Some(value)) => match FromPyObject::extract_bound(&value) {
				Ok(value) => value,
				Err(e) => {
					e.print(py);
					panic!("Unable to convert `{}` to `{}`", name, std::any::type_name::<T>());
				}
			},
		})
	}

	/// Set a global variable in the context.
	///
	/// This function panics if the conversion fails.
	pub fn set<T: for<'p> IntoPyObject<'p>>(&self, name: &str, value: T) {
		Python::with_gil(|py| match self.globals().bind(py).set_item(name, value) {
			Ok(()) => (),
			Err(e) => {
				e.print(py);
				panic!("Unable to set `{}` from a `{}`", name, std::any::type_name::<T>());
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
			let name = obj.getattr("__name__").expect("Missing __name__");
			if let Err(e) = self.globals().bind(py).set_item(name, obj) {
				e.print(py);
				panic!("Unable to add wrapped function");
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
	pub fn run<F: FnOnce(&Bound<PyDict>)>(&self, code: PythonBlock<F>) {
		Python::with_gil(|py| self.run_with_gil(py, code));
	}

	pub(crate) fn run_with_gil<F: FnOnce(&Bound<PyDict>)>(&self, py: Python<'_>, code: PythonBlock<F>) {
		(code.set_variables)(self.globals().bind(py));
		match run_python_code(py, self, code.bytecode) {
			Ok(_) => (),
			Err(e) => {
				e.print(py);
				panic!("{}", "python!{...} failed to execute");
			}
		}
	}
}
