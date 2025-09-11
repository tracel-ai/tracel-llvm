pub type BundlerResult<T> = std::result::Result<T, BundlingError>;

#[derive(Debug)]
pub enum BundlingError {
    UnsupportedSystem,
    IoError(std::io::Error),
    NetworkError(reqwest::Error),
    /// UTF-8 decoding failed while reading tool output.
    Utf8(std::str::Utf8Error),
    /// External tool executed but failed with a non-zero status.
    ToolExit {
        path: String,
        status: i32,
        stderr: String,
    },
}

impl std::error::Error for BundlingError {}

impl std::fmt::Display for BundlingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BundlingError::UnsupportedSystem => write!(f, "Unsupported system"),
            BundlingError::IoError(error) => write!(f, "{error}"),
            BundlingError::NetworkError(error) => write!(f, "{error}"),
            BundlingError::Utf8(error) => write!(f, "{error}"),
            BundlingError::ToolExit {
                path,
                status,
                stderr,
            } => {
                if stderr.is_empty() {
                    write!(f, "`{path}` exited with status {status}")
                } else {
                    write!(f, "`{path}` exited with status {status}: {stderr}")
                }
            }
        }
    }
}

impl From<std::io::Error> for BundlingError {
    fn from(value: std::io::Error) -> Self {
        BundlingError::IoError(value)
    }
}
impl From<reqwest::Error> for BundlingError {
    fn from(value: reqwest::Error) -> Self {
        BundlingError::NetworkError(value)
    }
}
impl From<std::str::Utf8Error> for BundlingError {
    fn from(value: std::str::Utf8Error) -> Self {
        BundlingError::Utf8(value)
    }
}
