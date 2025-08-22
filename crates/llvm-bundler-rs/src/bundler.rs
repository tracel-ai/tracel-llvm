use std::{
    env::set_var,
    error, fmt,
    fs::{self, create_dir, exists},
    io,
    path::PathBuf,
    time::Duration,
};

use bytes::Bytes;
use dirs::data_local_dir;
use liblzma::bufread::XzDecoder;
use tar::Archive;

const LINUX_ARTIFACT_URL: &str = "https://github.com/marcantoinem/llvm-bundler-rs/releases/download/master/llvm-linux-x64.tar.xz";
const LLVM_CACHE_PREFIX: &str = "llvm";
const FINISH_FILE_MUTEX: &str = "complete";

#[derive(Debug)]
pub enum BundlingError {
    UnsupportedSystem,
    IoError(io::Error),
    NetworkError(reqwest::Error),
}

impl error::Error for BundlingError {}

impl fmt::Display for BundlingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BundlingError::UnsupportedSystem => write!(f, "Unsupported system"),
            BundlingError::IoError(error) => write!(f, "{}", error),
            BundlingError::NetworkError(error) => write!(f, "{}", error),
        }
    }
}

impl From<io::Error> for BundlingError {
    fn from(value: io::Error) -> Self {
        BundlingError::IoError(value)
    }
}

impl From<reqwest::Error> for BundlingError {
    fn from(value: reqwest::Error) -> Self {
        BundlingError::NetworkError(value)
    }
}

pub type Result<T> = std::result::Result<T, BundlingError>;

pub fn llvm_path() -> Result<PathBuf> {
    data_local_dir()
        .map(|p| p.join(LLVM_CACHE_PREFIX))
        .ok_or(BundlingError::UnsupportedSystem)
}

fn decompress_tar_xz_stream(data: Bytes) -> Result<()> {
    let cursor = std::io::Cursor::new(data);
    let decoder = XzDecoder::new_parallel(cursor);
    let mut archive = Archive::new(decoder);
    let local_dir = data_local_dir().ok_or(BundlingError::UnsupportedSystem)?;
    archive.unpack(&local_dir)?;
    fs::write(llvm_path()?.join(FINISH_FILE_MUTEX), b"")?;
    Ok(())
}

pub fn bundle_cache() -> Result<()> {
    let llvm_path = llvm_path()?;
    if !exists(&llvm_path).unwrap_or(false) {
        create_dir(&llvm_path).unwrap();
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60 * 5))
            .build()?;
        let response = client.get(LINUX_ARTIFACT_URL).send()?.bytes()?;
        decompress_tar_xz_stream(response)?;
    } else if !exists(llvm_path.join(FINISH_FILE_MUTEX)).unwrap_or(false) {
        // Is already downloading and extracting
        while !exists(llvm_path.join(FINISH_FILE_MUTEX)).unwrap_or(false) {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    let libclang_path = llvm_path.join("lib");
    let include_path = llvm_path.join("include");

    //SAFETY: The build.rs should not be multithreaded at this point.
    unsafe {
        set_var("TABLEGEN_200_PREFIX", &llvm_path);
        set_var("MLIR_SYS_200_PREFIX", &llvm_path);
        set_var("LIBCLANG_PATH", libclang_path);
        set_var("LLVM_INCLUDE_DIRECTORY", include_path);
    }
    Ok(())
}
