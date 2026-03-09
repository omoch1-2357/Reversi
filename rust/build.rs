use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const MODEL_ENV_VAR: &str = "REVERSI_MODEL_PATH";
const DEFAULT_MODEL_PATH: &str = "src/ai/weights.bin";
const OUT_FILE_NAME: &str = "embedded_weights.bin";

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set"));
    let default_model_path = manifest_dir.join(DEFAULT_MODEL_PATH);
    let selected_model_path = resolve_model_path(&manifest_dir, &default_model_path);
    let out_path =
        PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set")).join(OUT_FILE_NAME);

    println!("cargo::rerun-if-env-changed={MODEL_ENV_VAR}");
    println!(
        "cargo::rerun-if-changed={}",
        selected_model_path.as_os_str().to_string_lossy()
    );

    if let Err(error) = fs::copy(&selected_model_path, &out_path) {
        panic!(
            "failed to copy model from '{}' to '{}': {error}",
            selected_model_path.display(),
            out_path.display()
        );
    }
}

fn resolve_model_path(manifest_dir: &Path, default_model_path: &Path) -> PathBuf {
    match env::var_os(MODEL_ENV_VAR) {
        Some(raw_path) => {
            let raw_path = PathBuf::from(raw_path);
            let resolved = if raw_path.is_absolute() {
                raw_path
            } else {
                manifest_dir.join(raw_path)
            };

            if !resolved.is_file() {
                panic!(
                    "{MODEL_ENV_VAR} must point to an existing weights.bin file, got '{}'",
                    resolved.display()
                );
            }

            resolved
        }
        None => default_model_path.to_path_buf(),
    }
}
