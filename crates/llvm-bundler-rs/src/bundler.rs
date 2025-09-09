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

const TRACEL_LLVM_ARTIFACT_BASE_URL: &str = "https://github.com/tracel-ai/tracel-llvm/releases/download";
const TRACEL_LLVM_CACHE_PREFIX: &str = "tracel-llvm";
const TRACEL_LLVM_FINISH_FILE_MUTEX: &str = "complete";
const TRACEL_LLVM_RELEASE_NUMBER: &str = "1";
const TRACEL_LLVM_VERSION: &str = "20.1.4";

pub enum OperatingSystem {
    Linux,
    MacOs,
    Windows,
}

impl OperatingSystem {
    // This will trigger a compilation error if the OS is not supported
    fn current() -> Self {
        #[cfg(target_os = "linux")]
        return Self::Linux;
        #[cfg(target_os = "macos")]
        return Self::MacOs;
        #[cfg(target_os = "windows")]
        return Self::Windows;
    }

    fn filename(self) -> &'static str {
        match self {
            OperatingSystem::Linux => "linux-x64.tar.xz",
            OperatingSystem::MacOs => "macos-AArch64.tar.xz",
            OperatingSystem::Windows => "windows-x64.tar.xz",
        }
    }

    pub fn artifact_url(self) -> String {
        let filename = self.filename();
        format!("{TRACEL_LLVM_ARTIFACT_BASE_URL}/v{TRACEL_LLVM_VERSION}-{TRACEL_LLVM_RELEASE_NUMBER}/{filename}")
    }
}

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
            BundlingError::IoError(error) => write!(f, "{error}"),
            BundlingError::NetworkError(error) => write!(f, "{error}"),
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
    let directory = format!("{TRACEL_LLVM_CACHE_PREFIX}-{TRACEL_LLVM_VERSION}-{TRACEL_LLVM_RELEASE_NUMBER}");
    data_local_dir()
        .map(|p| p.join(directory))
        .ok_or(BundlingError::UnsupportedSystem)
}

fn decompress_tar_xz_stream(data: Bytes) -> Result<()> {
    let cursor = std::io::Cursor::new(data);
    let decoder = XzDecoder::new_parallel(cursor);
    let mut archive = Archive::new(decoder);
    let local_dir = data_local_dir().ok_or(BundlingError::UnsupportedSystem)?;
    archive.unpack(&local_dir)?;
    fs::write(escape_pathbuf(&llvm_path()?.join(TRACEL_LLVM_FINISH_FILE_MUTEX)), b"")?;
    Ok(())
}

fn escape_pathbuf(path: &PathBuf) -> String {
    format!(r#""{}""#, path.to_str().unwrap())
}

pub fn bundle_cache() -> Result<()> {
    let llvm_path = llvm_path()?;
    let opsys = OperatingSystem::current();
    if !exists(&llvm_path).unwrap_or(false) {
        create_dir(&llvm_path).unwrap();
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60 * 5))
            .build()?;
        let response = client.get(opsys.artifact_url()).send()?.bytes()?;
        decompress_tar_xz_stream(response)?;
    } else if !exists(llvm_path.join(TRACEL_LLVM_FINISH_FILE_MUTEX)).unwrap_or(false) {
        // Is already downloading and extracting
        while !exists(llvm_path.join(TRACEL_LLVM_FINISH_FILE_MUTEX)).unwrap_or(false) {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    let libclang_path = llvm_path.join("lib");
    let include_path = llvm_path.join("include");

    //SAFETY: The build.rs should not be multithreaded at this point.
    unsafe {
        set_var("TABLEGEN_200_PREFIX", escape_pathbuf(&llvm_path));
        set_var("MLIR_SYS_200_PREFIX", escape_pathbuf(&llvm_path));
        set_var("LIBCLANG_PATH", escape_pathbuf(&libclang_path));
        set_var("LLVM_INCLUDE_DIRECTORY", escape_pathbuf(&include_path));
    }
    Ok(())
}
