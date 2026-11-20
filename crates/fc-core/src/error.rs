use std::path::PathBuf;

/// One error type for the whole core. `Display` text is shown to users after
/// the UI maps `code()` to a localized message, so keep messages short.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cancelled")]
    Cancelled,
    /// An external tool (ffmpeg, yt-dlp, deno, pdfium) is required but not installed.
    #[error("missing tool: {0}")]
    MissingTool(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("input not found: {0}")]
    NotFound(PathBuf),
    #[error("too large: {0}")]
    TooLarge(String),
    #[error("network: {0}")]
    Network(String),
    #[error("checksum mismatch for {0}")]
    Checksum(String),
    #[error("tool failed: {0}")]
    ToolFailed(String),
    /// A request or configuration that breaks a rule (limits, unknown references, an operation
    /// the queue cannot run).
    #[error("invalid: {0}")]
    Invalid(String),
    #[error("{0}")]
    Failed(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Error {
    /// Stable machine code for the UI (i18n key suffix: `error.<code>`).
    pub fn code(&self) -> &'static str {
        match self {
            Error::Cancelled => "cancelled",
            Error::MissingTool(_) => "missingTool",
            Error::Unsupported(_) => "unsupported",
            Error::NotFound(_) => "notFound",
            Error::TooLarge(_) => "tooLarge",
            Error::Network(_) => "network",
            Error::Checksum(_) => "checksum",
            Error::ToolFailed(_) => "toolFailed",
            Error::Invalid(_) => "invalid",
            Error::Failed(_) => "failed",
            Error::Io(_) => "io",
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
