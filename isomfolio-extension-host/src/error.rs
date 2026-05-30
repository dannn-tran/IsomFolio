use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("extension error: {0}")]
    Extension(String),
    #[error("install error: {0}")]
    Install(String),
}

impl Error {
    pub fn into_string(self) -> String {
        self.to_string()
    }
}
