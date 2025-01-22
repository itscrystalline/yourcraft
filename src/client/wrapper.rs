use pyo3::prelude::*;
use pyo3::prepare_freethreaded_python;
use pyo3::types::PyModule;
use std::ffi::CString;
use std::fs::read_to_string;
use std::path::Path;

fn main() -> PyResult<()> {
    let python_path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src/client/py"));
    let main = CString::new(read_to_string(python_path.join("main.py"))?)?;

    prepare_freethreaded_python();
    Python::with_gil(|py| -> PyResult<Py<PyAny>> {
        let py_path = py.import("sys")?.getattr("path")?;
        let sys_path = py_path.downcast()?;
        sys_path.insert(0, &python_path)?;
        let app: Py<PyAny> = PyModule::from_code(py, &main, c"", c"")?
            .getattr("main")?
            .into();
        app.call0(py)
    })?;
    Ok(())
}