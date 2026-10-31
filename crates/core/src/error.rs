/// Every error is a sentence a player can act on; the variants only keep the cause.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Msg(String),
    #[error("File error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Network error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Unexpected data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Broken zip file: {0}")]
    Zip(#[from] zip::result::ZipError),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn msg(text: impl Into<String>) -> Error {
    Error::Msg(text.into())
}
