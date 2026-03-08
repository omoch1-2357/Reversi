use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

#[pyfunction(signature = (
    games,
    alpha = 0.01,
    lambda_ = 0.7,
    epsilon = 0.1,
    seed = 42,
    progress_interval = 0,
    progress_callback = None
))]
fn train_to_bytes(
    games: usize,
    alpha: f32,
    lambda_: f32,
    epsilon: f64,
    seed: u64,
    progress_interval: usize,
    progress_callback: Option<Py<PyAny>>,
) -> PyResult<Vec<u8>> {
    let mut callback_error: Option<PyErr> = None;
    let mut progress = |completed: usize, total: usize, elapsed: f64| -> Result<(), String> {
        if let Some(callback) = progress_callback.as_ref() {
            Python::with_gil(|py| {
                callback
                    .bind(py)
                    .call1((completed, total, elapsed))
                    .map_err(|err| {
                        callback_error = Some(err);
                        "python progress callback failed".to_string()
                    })?;
                Ok(())
            })
        } else {
            Ok(())
        }
    };

    let result = if progress_callback.is_some() {
        reversi::training::train_to_bytes(
            games,
            alpha,
            lambda_,
            epsilon,
            seed,
            progress_interval,
            Some(&mut progress),
        )
    } else {
        reversi::training::train_to_bytes(
            games,
            alpha,
            lambda_,
            epsilon,
            seed,
            progress_interval,
            None,
        )
    };

    match result {
        Ok(bytes) => Ok(bytes),
        Err(err) => {
            if let Some(pyerr) = callback_error {
                Err(pyerr)
            } else {
                Err(PyRuntimeError::new_err(err))
            }
        }
    }
}

#[pymodule]
fn _reversi_training(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(train_to_bytes, module)?)?;
    Ok(())
}
