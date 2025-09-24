include!("src/config.rs");

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, create_dir_all},
    io::{BufReader, Read},
    time::Duration,
};
use walkdir::WalkDir;

use liblzma::bufread::XzDecoder;
use tar::Archive;

const TRACEL_LLVM_ARTIFACT_BASE_URL: &str =
    "https://github.com/tracel-ai/tracel-llvm/releases/download";

type AnyResult<T> = Result<T>;

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

    fn checksum_filename(&self) -> String {
        self.filename().replace(".tar.xz", ".checksums.json")
    }

    pub fn artifact_url(&self) -> String {
        let filename = self.filename();
        format!(
            "{TRACEL_LLVM_ARTIFACT_BASE_URL}/v{TRACEL_LLVM_FULL_VERSION}/{filename}"
        )
    }

    pub fn checksum_url(&self) -> String {
        let filename = self.checksum_filename();
        format!(
            "{TRACEL_LLVM_ARTIFACT_BASE_URL}/v{TRACEL_LLVM_FULL_VERSION}/{filename}"
        )
    }

    /// Returns the same cache directory on all OSes for consistency sake
    fn artifact_cache_dir() -> AnyResult<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        let base = home.join(".cache").join("tracel");
        create_dir_all(&base)?;
        Ok(base)
    }

    fn cache_filename(&self) -> String {
        format!("tracel-llvm-{TRACEL_LLVM_FULL_VERSION}-{}", self.filename())
    }

    fn checksum_cache_filename(&self) -> String {
        format!("tracel-llvm-{TRACEL_LLVM_FULL_VERSION}-{}", self.checksum_filename())
    }

    fn artifact_cache_path(&self) -> AnyResult<PathBuf> {
        let base = Self::artifact_cache_dir()?;
        Ok(base.join(self.cache_filename()))
    }

    fn checksum_cache_path(&self) -> AnyResult<PathBuf> {
        let base = Self::artifact_cache_dir()?;
        Ok(base.join(self.checksum_cache_filename()))
    }
}

struct RollbackDir {
    path: PathBuf,
    armed: bool,
}
impl RollbackDir {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }
    fn commit(mut self) {
        self.armed = false;
    }
}
impl Drop for RollbackDir {
    fn drop(&mut self) {
        if self.armed && self.path.exists() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[derive(Deserialize)]
struct Sidecar {
    archive_sha256: String,
    content_sha256: String,
}

/// Download (overwriting) to a path.
fn download_to_path(url: &str, dest: &Path) -> AnyResult<()> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60 * 5))
        .build()?;
    let mut resp = client.get(url).send()?.error_for_status()?;
    if let Some(parent) = dest.parent() {
        create_dir_all(parent)?;
    }
    let mut f = File::create(dest)?;
    std::io::copy(&mut resp, &mut f)?;
    Ok(())
}

fn file_sha256_hex(path: &Path) -> AnyResult<String> {
    let f = File::open(path)?;
    let mut r = BufReader::with_capacity(128 * 1024, f);
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = r.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Deterministic directory digest:
/// For each regular file under `root`, in lexicographic order by forward-slash
/// relative path, feed: PATH\n + SIZE\n + BYTES into a single SHA-256.
fn directory_content_sha256_hex(root: &Path) -> AnyResult<String> {
    let mut files = Vec::<PathBuf>::new();
    for e in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if e.file_type().is_file() {
            files.push(e.into_path());
        }
    }
    files.sort_by(|a, b| {
        let ra = a.strip_prefix(root).unwrap();
        let rb = b.strip_prefix(root).unwrap();
        ra.to_string_lossy()
            .replace('\\', "/")
            .cmp(&rb.to_string_lossy().replace('\\', "/"))
    });

    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];

    for file in files {
        let rel = file
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        hasher.update(rel.as_bytes());
        hasher.update(b"\n");

        let size = fs::metadata(&file)?.len().to_string();
        hasher.update(size.as_bytes());
        hasher.update(b"\n");

        let mut r = BufReader::new(File::open(&file)?);
        loop {
            let n = r.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Unpack the tar.xz bytes into `dest_dir`
fn decompress_tar_xz_file_to(archive_path: &Path, dest_dir: &Path) -> AnyResult<()> {
    let f = File::open(archive_path)?;
    let reader = BufReader::new(f);
    let decoder = XzDecoder::new_parallel(reader);
    let mut archive = Archive::new(decoder);
    archive.unpack(dest_dir)?;
    Ok(())
}

pub fn bundle_cache() -> AnyResult<()> {
    // 0) Already installed?
    let llvm_path = llvm_path()?;
    if llvm_path.exists() {
        // This check is lightweight, but we go to great lengths to ensure that if the
        // installation process completes fully, the install is reliable.
        return Ok(());
    }

    // 1) Prepare installation
    let parent = llvm_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid llvm_path"))?;
    create_dir_all(parent)?;
    // Rollback guard: if anything fails after extraction, remove llvm_path
    let rollback = RollbackDir::new(llvm_path.clone());
    let opsys = OperatingSystem::current();

    // 2) Download and load sidecar file with checksums
    let checksum_path = opsys.checksum_cache_path()?;
    download_to_path(&opsys.checksum_url(), &checksum_path)
        .with_context(|| format!("downloading {}", opsys.checksum_url()))?;
    let mut sidecar_text = fs::read_to_string(&checksum_path)
        .with_context(|| format!("reading {}", checksum_path.display()))?;
    // Windows fix: strip UTF-8 BOM if present (can happen with some versions of powershell)
    if sidecar_text.starts_with('\u{FEFF}') {
        sidecar_text = sidecar_text.trim_start_matches('\u{FEFF}').to_string();
    }
    let sidecar: Sidecar = serde_json::from_str(&sidecar_text)
        .with_context(|| "parsing checksum sidecar JSON")?;

    // 3) Download bundle if required (i.e. it does not exist or its checksum does not match)
    let archive_path = opsys.artifact_cache_path()?;
    if archive_path.exists() {
        let local = file_sha256_hex(&archive_path)?;
        if local != sidecar.archive_sha256 {
            // re-download and re-check
            download_to_path(&opsys.artifact_url(), &archive_path)
                .with_context(|| format!("re-downloading {}", opsys.artifact_url()))?;
            let again = file_sha256_hex(&archive_path)?;
            if again != sidecar.archive_sha256 {
                bail!(
                    "Archive checksum mismatch after re-download.\n  expected: {}\n  got:      {}\nURL: {}",
                    sidecar.archive_sha256,
                    again,
                    opsys.artifact_url()
                );
            }
        }
    } else {
        download_to_path(&opsys.artifact_url(), &archive_path)
            .with_context(|| format!("downloading {}", opsys.artifact_url()))?;
        let got = file_sha256_hex(&archive_path)?;
        if got != sidecar.archive_sha256 {
            bail!(
                "Archive checksum mismatch after download.\n  expected: {}\n  got:      {}\nURL: {}",
                sidecar.archive_sha256,
                got,
                opsys.artifact_url()
            );
        }
    }

    // 4) Extract bundle
    // The tarball contains exactly one top-level dir: tracel-llvm-<ver>-<rel>
    decompress_tar_xz_file_to(&archive_path, parent)?;
    // The expected directory must now exist
    if !llvm_path.exists() {
        bail!(
            "Extraction completed but expected directory not found: {}",
            llvm_path.display()
        );
    }

    // 5) Verify extracted content checksum on the final destination folder
    let content = directory_content_sha256_hex(&llvm_path)?;
    if content != sidecar.content_sha256 {
        bail!(
            "Extracted content checksum mismatch.\n  expected: {}\n  got:      {}\n\n\
             Please re-run the build. If the issue persists, contact the repository admins.",
            sidecar.content_sha256,
            content
        );
    }

    // Success, don't clean up
    rollback.commit();
    Ok(())
}
