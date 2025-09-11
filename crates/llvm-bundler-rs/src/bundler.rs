use std::{
    env::set_var,
    fs::{self, create_dir},
    path::PathBuf,
    time::Duration,
};

use bytes::Bytes;
use dirs::data_local_dir;
use liblzma::bufread::XzDecoder;
use tar::Archive;

use crate::error::{BundlerResult, BundlingError};

const TRACEL_LLVM_ARTIFACT_BASE_URL: &str =
    "https://github.com/tracel-ai/tracel-llvm/releases/download";
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
        format!(
            "{TRACEL_LLVM_ARTIFACT_BASE_URL}/v{TRACEL_LLVM_VERSION}-{TRACEL_LLVM_RELEASE_NUMBER}/{filename}"
        )
    }
}

pub fn llvm_path() -> BundlerResult<PathBuf> {
    let directory =
        format!("{TRACEL_LLVM_CACHE_PREFIX}-{TRACEL_LLVM_VERSION}-{TRACEL_LLVM_RELEASE_NUMBER}");
    data_local_dir()
        .map(|p| p.join(directory))
        .ok_or(BundlingError::UnsupportedSystem)
}

fn decompress_tar_xz_stream(data: Bytes) -> BundlerResult<()> {
    let cursor = std::io::Cursor::new(data);
    let decoder = XzDecoder::new_parallel(cursor);
    let mut archive = Archive::new(decoder);
    let local_dir = data_local_dir().ok_or(BundlingError::UnsupportedSystem)?;
    archive.unpack(&local_dir)?;
    fs::write(
        super::utils::quote_path(&llvm_path()?.join(TRACEL_LLVM_FINISH_FILE_MUTEX)),
        b"",
    )?;
    Ok(())
}

pub fn bundle_cache() -> BundlerResult<()> {
    let llvm_path = llvm_path()?;
    let opsys = OperatingSystem::current();

    if !llvm_path.exists() {
        create_dir(&llvm_path)?;
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60 * 5))
            .build()?;
        let response = client.get(opsys.artifact_url()).send()?.bytes()?;
        decompress_tar_xz_stream(response)?;
    } else if !llvm_path.join(TRACEL_LLVM_FINISH_FILE_MUTEX).exists() {
        // Is already downloading and extracting
        while !llvm_path.join(TRACEL_LLVM_FINISH_FILE_MUTEX).exists() {
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    let libclang_path = llvm_path.join("lib");
    let include_path = llvm_path.join("include");

    // SAFETY: The build.rs should not be multithreaded at this point.
    unsafe {
        set_var("TABLEGEN_200_PREFIX", &llvm_path);
        set_var("MLIR_SYS_200_PREFIX", &llvm_path);
        set_var("LIBCLANG_PATH", &libclang_path);
        set_var("LLVM_INCLUDE_DIRECTORY", &include_path);
    }
    Ok(())
}
