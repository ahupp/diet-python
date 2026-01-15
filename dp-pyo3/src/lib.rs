use dp_transform::transform_to_string_without_attribute_lowering;
use pyo3::prelude::*;

#[pyfunction]
fn transform_source(source: &str, ensure: Option<bool>) -> PyResult<String> {
    let ensure = ensure.unwrap_or(true);
    match transform_to_string_without_attribute_lowering(source, ensure) {
        Ok(output) => Ok(output),
        Err(err) => Err(pyo3::exceptions::PySyntaxError::new_err(err.to_string())),
    }
}

#[pymodule]
fn diet_python(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(transform_source, module)?)?;
    Ok(())
}
