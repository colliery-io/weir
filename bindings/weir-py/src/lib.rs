use pyo3::prelude::*;

/// A simple test function.
#[pyfunction]
fn hello() -> String {
    "Hello from weir!".to_string()
}

/// Python module for weir.
#[pymodule]
fn weir(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(hello, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello() {
        assert_eq!(hello(), "Hello from weir!");
    }
}
