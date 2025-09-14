include!("src/config.rs");

use std::{
    fs::{self, create_dir_all},
    time::Duration,
};

use bytes::Bytes;
use liblzma::bufread::XzDecoder;
use tar::Archive;

const TRACEL_LLVM_ARTIFACT_BASE_URL: &str =
    "https://github.com/tracel-ai/tracel-llvm/releases/download";

type AnyResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> AnyResult<()> {
    bundle_cache()?;
    Ok(())
}

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

    fn filename(&self) -> &'static str {
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

    /// Returns the same cache directory on all OSes for consistency sake
    fn artifact_cache_dir() -> AnyResult<PathBuf> {
        let home = dirs::home_dir()
            .ok_or("Could not determine home directory")?;
        let base = home.join(".cache").join("tracel");
        create_dir_all(&base)?;
        Ok(base)
    }

    fn cache_filename(&self) -> String {
        format!(
            "tracel-llvm-{ver}-{rel}-{}",
            self.filename(),
            ver = TRACEL_LLVM_VERSION,
            rel = TRACEL_LLVM_RELEASE_NUMBER
        )
    }

    fn artifact_cache_path(&self) -> AnyResult<PathBuf> {
        let base = Self::artifact_cache_dir()?;
        Ok(base.join(self.cache_filename()))
    }
}

/// We try to cleanup temporary files whatever happens
struct DirGuard {
    path: PathBuf,
}
impl DirGuard {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}
impl Drop for DirGuard {
    fn drop(&mut self) {
        // Best-effort cleanup; ignore errors
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Ensure the artifact exists in ~/.cache/tracel, download if required.
fn ensure_cached_artifact(os: &OperatingSystem) -> AnyResult<PathBuf> {
    let path = os.artifact_cache_path()?;
    if path.exists() {
        println!("cargo:warning=Using cached LLVM artifact at {}", path.display());
        return Ok(path);
    }

    println!("cargo:warning=Downloading LLVM artifact to cache…");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60 * 5))
        .build()?;

    let mut resp = client.get(os.artifact_url()).send()?.error_for_status()?;
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(&path)?;
    std::io::copy(&mut resp, &mut file)?;
    println!("cargo:warning=Saved {}", path.display());
    Ok(path)
}

/// Unpack the tar.xz bytes into `dest_dir`
fn decompress_tar_xz_file_to(archive_path: &Path, dest_dir: &Path) -> AnyResult<()> {
    let f = std::fs::File::open(archive_path)?;
    let reader = std::io::BufReader::new(f);
    let decoder = XzDecoder::new_parallel(reader);
    let mut archive = Archive::new(decoder);
    archive.unpack(dest_dir)?;
    Ok(())
}

pub fn bundle_cache() -> AnyResult<()> {
    let llvm_path = llvm_path()?; // from your config
    println!(
        "cargo:warning=LLVM CACHE PATH: {}",
        llvm_path.to_string_lossy()
    );

    if !llvm_path.exists() {
        let temp_dir = llvm_path.with_extension("partial");
        if temp_dir.exists() {
            fs::remove_dir_all(&temp_dir)?;
        }
        create_dir_all(&temp_dir)?;
        let _guard = DirGuard::new(temp_dir.clone());

        // Get or download the artifact to ~/.cache/tracel/… then unpack from the cached file
        let opsys = OperatingSystem::current();
        let cached_archive = ensure_cached_artifact(&opsys)?;

        // Unpack into temp_dir and move the extracted directory to final destination (mile 180)
        decompress_tar_xz_file_to(&cached_archive, &temp_dir)?;
        if let Some(parent) = llvm_path.parent() {
            create_dir_all(parent)?;
        }
        // Important: the archive must have exactly one top-level directory in the tarball.
        let mut dirs = Vec::new();
        for entry in fs::read_dir(&temp_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                dirs.push(entry.path());
            }
        }
        if dirs.len() != 1 {
            // errors out if the archive has not the expected format (one top-level directory)
            return Err(Box::<dyn std::error::Error + Send + Sync>::from(
                "Archive should contain a single top-level directory",
            ));
        }
        fs::rename(&dirs[0], &llvm_path)?;
    }
    Ok(())
}
