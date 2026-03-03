use std::path::PathBuf;

pub const PKG_NAME: &str = env!("CARGO_PKG_NAME");

pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap().join(PKG_NAME)
}

pub fn model_dir() -> PathBuf {
    dirs::state_dir().unwrap().join(PKG_NAME).join("model")
}
