use std::ffi::{CStr, CString};
use std::fs::read_to_string;
use std::path::Path;
use include_dir::{include_dir, Dir};
use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::prepare_freethreaded_python;
use pyo3::types::PyModule;

fn main() -> PyResult<()> {
    static PYTHON: Dir = include_dir!("$CARGO_MANIFEST_DIR/src/client/py");
    let main = PYTHON.get_file("main.py").unwrap();
    let main_contents = CString::new(main.contents_utf8().unwrap())?;
    
    prepare_freethreaded_python();
    Python::with_gil(|py| {
        PyModule::from_code(
            py,
            &*main_contents,
            c_str!("main.py"),
            c"main",
        )?;
        Ok(())
    })
}