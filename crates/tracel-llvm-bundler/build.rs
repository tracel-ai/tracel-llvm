include!("src/config.rs");

mod build_support {
    pub mod archive {
        include!("src/build_support/archive.rs");
    }
    pub mod checksums {
        include!("src/build_support/checksums.rs");
    }
}

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{
    fs::{File, create_dir_all},
    time::Duration,
};

type AnyResult<T> = Result<T>;

fn main() {
    // in xtask mode we skip the bundler installation alltogether
    if std::env::var_os("CARGO_FEATURE_XTASK").is_some() {
        println!("cargo:warning=xtask mode enabled, skipping bundle installation.");
        return;
    }

    if let Err(error) = bundle_cache() {
        eprintln!("{error}");
        std::process::exit(1);
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

pub fn bundle_cache() -> AnyResult<()> {
    let env = Environment::current();
    let llvm_path = llvm_path()?;

    let sentinel = env.sentinel_path(&llvm_path);
    println!("cargo:rerun-if-changed={}", sentinel.display());

    // 0) Already installed?
    if sentinel.exists() {
        return Ok(());
    }

    // 1) Prepare installation
    let parent = llvm_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid llvm_path"))?;
    create_dir_all(parent)?;
    let rollback = RollbackDir::new(llvm_path.clone());

    // 2) Download and parse checksums sidecar

    let skip_download = std::env::var_os("TRACEL_LLVM_BUNDLER_SKIP_DOWNLOAD").is_some();

    let checksum_path = env.checksum_cache_path()?;
    if !skip_download {
        download_to_path(&env.checksum_url(), &checksum_path)
            .with_context(|| format!("downloading {}", env.checksum_url()))?;
    }
    else {
        println!("cargo:warning=TRACEL_LLVM_BUNDLER_SKIP_DOWNLOAD is set, skipping checksum download.");
    }

    let mut sidecar_text = fs::read_to_string(&checksum_path)
        .with_context(|| format!("reading {}", checksum_path.display()))?;
    if sidecar_text.starts_with('\u{FEFF}') {
        sidecar_text = sidecar_text.trim_start_matches('\u{FEFF}').to_string();
    }
    let sidecar: Sidecar =
        serde_json::from_str(&sidecar_text).with_context(|| "parsing checksum sidecar JSON")?;

    // 3) Download bundle if required / verify archive checksum
    let archive_path = env.artifact_cache_path()?;
    if archive_path.exists() {
        let local = build_support::checksums::sha256_file_hex(&archive_path)?;
        if local != sidecar.archive_sha256 {
            download_to_path(&env.artifact_url(), &archive_path)
                .with_context(|| format!("re-downloading {}", env.artifact_url()))?;
            let again = build_support::checksums::sha256_file_hex(&archive_path)?;
            if again != sidecar.archive_sha256 {
                bail!(
                    "Archive checksum mismatch after re-download.\n  expected: {}\n  got:      {}\nURL: {}",
                    sidecar.archive_sha256,
                    again,
                    env.artifact_url()
                );
            }
        }
    } else {
        download_to_path(&env.artifact_url(), &archive_path)
            .with_context(|| format!("downloading {}", env.artifact_url()))?;
        let got = build_support::checksums::sha256_file_hex(&archive_path)?;
        if got != sidecar.archive_sha256 {
            bail!(
                "Archive checksum mismatch after download.\n  expected: {}\n  got:      {}\nURL: {}",
                sidecar.archive_sha256,
                got,
                env.artifact_url()
            );
        }
    }

    // 4) Extract bundle
    build_support::archive::decompress_tar_xz_file_to(&archive_path, parent)
        .with_context(|| "extracting tar.xz bundle should succeed")?;

    if !llvm_path.exists() {
        bail!(
            "Extraction completed but expected directory not found: {}",
            llvm_path.display()
        );
    }

    // 5) Verify extracted content checksum on final destination folder
    let content = build_support::checksums::sha256_tree_content_hex(&llvm_path)?;
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
