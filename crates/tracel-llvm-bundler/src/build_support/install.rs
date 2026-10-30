use std::{
    fs::{self, File, create_dir_all},
    path::{Path, PathBuf},
};

use anyhow::{Context as _, bail};
use serde::Deserialize;

use crate::{
    build_support::{archive::decompress_tar_xz_file_to, checksums},
    config::Environment,
};

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
fn download_to_path(url: &str, dest: &Path) -> anyhow::Result<()> {
    let response = ureq::get(url).call()?;
    if let Some(parent) = dest.parent() {
        create_dir_all(parent)?;
    }
    let mut f = File::create(dest)?;
    std::io::copy(&mut response.into_body().as_reader(), &mut f)?;
    Ok(())
}

/// Installs the bundle of `environment` at `llvm_path`, unless it is already there.
///
/// The download, the archive and the extracted tree are all verified against the
/// checksums sidecar published beside the archive.
pub fn install_bundle(environment: &Environment, llvm_path: &Path) -> anyhow::Result<()> {
    let sentinel = environment.sentinel_path(llvm_path);

    // 0) Already installed?
    if sentinel.exists() {
        return Ok(());
    }

    // 1) Prepare installation
    let parent = llvm_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid llvm_path"))?;
    create_dir_all(parent)?;
    let rollback = RollbackDir::new(llvm_path.to_path_buf());

    // 2) Download and parse checksums sidecar

    let skip_download = std::env::var_os("TRACEL_LLVM_BUNDLER_SKIP_CHECKSUM_DOWNLOAD").is_some();

    let checksum_path = environment.checksum_cache_path()?;
    if !skip_download {
        download_to_path(&environment.checksum_url(), &checksum_path)
            .with_context(|| format!("downloading {}", environment.checksum_url()))?;
    } else {
        println!(
            "cargo:warning=TRACEL_LLVM_BUNDLER_SKIP_CHECKSUM_DOWNLOAD is set, skipping checksum download."
        );
    }

    let mut sidecar_text = fs::read_to_string(&checksum_path)
        .with_context(|| format!("reading {}", checksum_path.display()))?;
    if sidecar_text.starts_with('\u{FEFF}') {
        sidecar_text = sidecar_text.trim_start_matches('\u{FEFF}').to_string();
    }
    let sidecar: Sidecar =
        serde_json::from_str(&sidecar_text).with_context(|| "parsing checksum sidecar JSON")?;

    // 3) Download bundle if required / verify archive checksum
    let archive_path = environment.artifact_cache_path()?;
    if archive_path.exists() {
        let local = checksums::sha256_file_hex(&archive_path)?;
        if local != sidecar.archive_sha256 {
            download_to_path(&environment.artifact_url(), &archive_path)
                .with_context(|| format!("re-downloading {}", environment.artifact_url()))?;
            let again = checksums::sha256_file_hex(&archive_path)?;
            if again != sidecar.archive_sha256 {
                bail!(
                    "Archive checksum mismatch after re-download.\n  expected: {}\n  got:      {}\nURL: {}",
                    sidecar.archive_sha256,
                    again,
                    environment.artifact_url()
                );
            }
        }
    } else {
        download_to_path(&environment.artifact_url(), &archive_path)
            .with_context(|| format!("downloading {}", environment.artifact_url()))?;
        let got = checksums::sha256_file_hex(&archive_path)?;
        if got != sidecar.archive_sha256 {
            bail!(
                "Archive checksum mismatch after download.\n  expected: {}\n  got:      {}\nURL: {}",
                sidecar.archive_sha256,
                got,
                environment.artifact_url()
            );
        }
    }

    // 4) Extract bundle
    decompress_tar_xz_file_to(&archive_path, parent)
        .with_context(|| "extracting tar.xz bundle should succeed")?;

    if !llvm_path.exists() {
        bail!(
            "Extraction completed but expected directory not found: {}",
            llvm_path.display()
        );
    }

    // 5) Verify extracted content checksum on final destination folder
    let content = checksums::sha256_tree_content_hex(llvm_path)?;
    if content != sidecar.content_sha256 {
        bail!(
            "Extracted content checksum mismatch.\n  expected: {}\n  got:      {}\n\n\
             Please re-run the build. If the issue persists, contact the repository admins.",
            sidecar.content_sha256,
            content
        );
    }

    // 6) Mark install complete
    if !sentinel.exists() {
        File::create(&sentinel)
            .with_context(|| format!("creating sentinel {}", sentinel.display()))?;
    }

    rollback.commit();
    Ok(())
}
