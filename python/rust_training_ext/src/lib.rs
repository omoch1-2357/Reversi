use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

#[pyfunction(signature = (
    games,
    alpha = 0.001,
    alpha_decay = "none",
    alpha_decay_start_game = 0,
    lambda_ = 0.7,
    epsilon = 0.1,
    seed = 42,
    threads = 1,
    initial_model = None,
    random_opening_plies = 0,
    progress_interval = 0,
    progress_callback = None
))]
fn train_to_bytes(
    py: Python<'_>,
    games: usize,
    alpha: f32,
    alpha_decay: &str,
    alpha_decay_start_game: usize,
    lambda_: f32,
    epsilon: f64,
    seed: u64,
    threads: usize,
    initial_model: Option<Vec<u8>>,
    random_opening_plies: usize,
    progress_interval: usize,
    progress_callback: Option<Py<PyAny>>,
) -> PyResult<Vec<u8>> {
    let alpha_decay = reversi::training::AlphaDecayStrategy::from_name(alpha_decay)
        .map_err(PyRuntimeError::new_err)?;
    let mut callback_error: Option<PyErr> = None;
    let mut progress = |completed: usize, total: usize, elapsed: f64| -> Result<(), String> {
        if let Some(callback) = progress_callback.as_ref() {
            Python::with_gil(|py| {
                callback.bind(py).call1((completed, total, elapsed)).map_err(|err| {
                    callback_error = Some(err);
                    "python progress callback failed".to_string()
                })?;
                Ok(())
            })
        } else {
            Ok(())
        }
    };

    let result = py.allow_threads(|| {
        if progress_callback.is_some() {
            reversi::training::train_to_bytes_with_alpha_decay(
                games,
                alpha,
                alpha_decay,
                alpha_decay_start_game,
                lambda_,
                epsilon,
                seed,
                threads,
                initial_model.as_deref(),
                random_opening_plies,
                progress_interval,
                Some(&mut progress),
            )
        } else {
            reversi::training::train_to_bytes_with_alpha_decay(
                games,
                alpha,
                alpha_decay,
                alpha_decay_start_game,
                lambda_,
                epsilon,
                seed,
                threads,
                initial_model.as_deref(),
                random_opening_plies,
                progress_interval,
                None,
            )
        }
    });

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

#[pyfunction(signature = (
    games,
    alpha = 0.001,
    alpha_decay = "none",
    alpha_decay_start_game = 0,
    lambda_ = 0.7,
    epsilon = 0.1,
    seed = 42,
    threads = 1,
    initial_model = None,
    random_opening_plies = 0,
    progress_interval = 0,
    progress_callback = None
))]
fn train_to_uncompressed_bytes(
    py: Python<'_>,
    games: usize,
    alpha: f32,
    alpha_decay: &str,
    alpha_decay_start_game: usize,
    lambda_: f32,
    epsilon: f64,
    seed: u64,
    threads: usize,
    initial_model: Option<Vec<u8>>,
    random_opening_plies: usize,
    progress_interval: usize,
    progress_callback: Option<Py<PyAny>>,
) -> PyResult<Vec<u8>> {
    let alpha_decay = reversi::training::AlphaDecayStrategy::from_name(alpha_decay)
        .map_err(PyRuntimeError::new_err)?;
    let mut callback_error: Option<PyErr> = None;
    let mut progress = |completed: usize, total: usize, elapsed: f64| -> Result<(), String> {
        if let Some(callback) = progress_callback.as_ref() {
            Python::with_gil(|py| {
                callback.bind(py).call1((completed, total, elapsed)).map_err(|err| {
                    callback_error = Some(err);
                    "python progress callback failed".to_string()
                })?;
                Ok(())
            })
        } else {
            Ok(())
        }
    };

    let result = py.allow_threads(|| {
        if progress_callback.is_some() {
            reversi::training::train_to_uncompressed_bytes_with_alpha_decay(
                games,
                alpha,
                alpha_decay,
                alpha_decay_start_game,
                lambda_,
                epsilon,
                seed,
                threads,
                initial_model.as_deref(),
                random_opening_plies,
                progress_interval,
                Some(&mut progress),
            )
        } else {
            reversi::training::train_to_uncompressed_bytes_with_alpha_decay(
                games,
                alpha,
                alpha_decay,
                alpha_decay_start_game,
                lambda_,
                epsilon,
                seed,
                threads,
                initial_model.as_deref(),
                random_opening_plies,
                progress_interval,
                None,
            )
        }
    });

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

#[pyfunction]
fn compress_model_bytes(py: Python<'_>, data: Vec<u8>) -> PyResult<Vec<u8>> {
    py.allow_threads(|| reversi::ai::ntuple::compress_model_bytes(&data))
        .map_err(PyRuntimeError::new_err)
}

#[pyfunction]
fn decompress_model_bytes(py: Python<'_>, data: Vec<u8>) -> PyResult<Vec<u8>> {
    py.allow_threads(|| {
        reversi::ai::ntuple::decompress_model_bytes(&data).map(|bytes| bytes.into_owned())
    })
    .map_err(PyRuntimeError::new_err)
}

#[pymodule]
fn _reversi_training(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(train_to_bytes, module)?)?;
    module.add_function(wrap_pyfunction!(train_to_uncompressed_bytes, module)?)?;
    module.add_function(wrap_pyfunction!(compress_model_bytes, module)?)?;
    module.add_function(wrap_pyfunction!(decompress_model_bytes, module)?)?;
    Ok(())
}
