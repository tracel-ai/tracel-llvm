use std::{
    fs,
    io::{self, Write as _},
    path::{Path, PathBuf},
};

use clap::{Args, Subcommand};
use tracel_llvm_bundler::build_support::{
    archive::create_tar_xz,
    checksums::{sha256_file_hex, sha256_tree_content_hex},
};
use tracel_xtask::prelude::{anyhow::Context as _, *};

use crate::utils::tblgen_shim::{build_and_install_ctablegen_shim, CTableGenShimConfig};

use super::BundleWorkspace;

#[derive(Args)]
pub struct BundleCmdArgs {
    #[command(subcommand)]
    cmd: BundleSubCmd,
}

#[derive(Subcommand)]
enum BundleSubCmd {
    /// Build the runtime bundle (LLVM+MLIR + CTableGen shim) then package it with checksums.
    Build(BundleBuildArgs),
    /// Delete the bundle workspace directory.
    Clean(BundleCleanArgs),
}

#[derive(Args)]
struct BundleBuildArgs {
    /// Workspace directory used for building
    #[arg(long, default_value = ".llvm")]
    workspace_dir: String,
}

#[derive(Args)]
struct BundleCleanArgs {
    /// Workspace directory used for building
    #[arg(long, default_value = ".llvm")]
    workspace_dir: String,

    /// Skip confirmation prompt
    #[arg(short = 'y', long)]
    yes: bool,
}

pub fn handle_command(args: BundleCmdArgs) -> anyhow::Result<()> {
    match args.cmd {
        BundleSubCmd::Build(a) => {
            let ws = BundleWorkspace::new(Path::new(&a.workspace_dir))?;
            build_runtime_bundle(&ws)
        }
        BundleSubCmd::Clean(a) => {
            let ws: PathBuf = a.workspace_dir.into();
            clean_bundle_workspace(&ws, a.yes)
        }
    }
}

fn build_runtime_bundle(ws: &BundleWorkspace) -> anyhow::Result<()> {
    crate::utils::process::require_tools(&["git", "cmake", "ninja"])?;

    ws.ensure_workspace_dir()?;
    // Clean up workspace
    for dir in [
        &ws.llvm_project_dir,
        &ws.bundle_build_dir,
        &ws.bundle_install_dir,
        &ws.clang_build_dir,
        &ws.clang_install_dir,
    ] {
        if dir.exists() {
            fs::remove_dir_all(dir).with_context(|| format!("Should delete '{}'", dir.display()))?;
        }
    }
    ws.clone_llvm_project_fresh()?;
    ws.build_mlir_project()?;
    // Keep only llvm-config in bin directory.
    prune_bin_to_llvm_config(ws)?;
    // Build  CTableGen shim into the bundle lib directory.
    group_info!("Bundle: build CTableGen shim");
    let shim_cfg = CTableGenShimConfig {
        repo_root: repo_root()?,
        bundle_install_dir: ws.bundle_install_dir.clone(),
        build_dir: ws.workspace_dir.join(".tblgen_shim_build"),
    };
    build_and_install_ctablegen_shim(shim_cfg)?;
    endgroup!();
    // Cleanup extra installed content
    group_info!("Bundle: cleanup");
    cleanup_bundle(ws)?;
    endgroup!();
    // Package and checksums.
    group_info!("Bundle: package + checksums");
    {
        let pkg_dir_name = format!("tracel-llvm-{}-{}", ws.version, ws.release_number);

        let out_name = format!("{}.tar.xz", ws.platform.archive_stem());
        let out_archive = ws.workspace_dir.join(&out_name);

        if out_archive.exists() {
            fs::remove_file(&out_archive)?;
        }

        create_tar_xz(&out_archive, &ws.bundle_install_dir, &pkg_dir_name)?;

        let archive_sha = sha256_file_hex(&out_archive)?;
        let content_sha = sha256_tree_content_hex(&ws.bundle_install_dir)?;

        let sidecar = ws
            .workspace_dir
            .join(format!("{}.checksums.json", ws.platform.archive_stem()));
        let created_at_utc = chrono_utc_iso8601();

        let manifest = serde_json::json!({
            "version": ws.version,
            "release_number": ws.release_number,
            "platform": ws.platform.archive_stem(),
            "created_at_utc": created_at_utc,
            "archive_sha256": archive_sha,
            "content_sha256": content_sha,
        });
        fs::write(&sidecar, serde_json::to_vec_pretty(&manifest)?)?;

        group_info!("Bundle outputs");
        println!("Install dir: {}", ws.bundle_install_dir.display());
        println!("Archive:     {}", out_archive.display());
        println!("Sidecar:     {}", sidecar.display());
        endgroup!();
    }
    endgroup!();

    Ok(())
}

fn prune_bin_to_llvm_config(ws: &BundleWorkspace) -> anyhow::Result<()> {
    group_info!("Bundle: prune bin/ to only llvm-config");

    let llvm_config = if cfg!(windows) {
        "llvm-config.exe"
    } else {
        "llvm-config"
    };

    // Delete everything in <install>/bin (files only), then copy built llvm-config.
    fs::create_dir_all(&ws.bundle_bin_dir).with_context(|| "bundle bin dir should be created")?;

    if let Ok(entries) = fs::read_dir(&ws.bundle_bin_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                let _ = fs::remove_file(p);
            }
        }
    }

    let built_llvm_config = ws.bundle_build_dir.join("bin").join(llvm_config);
    if !built_llvm_config.is_file() {
        return Err(anyhow::anyhow!(
            "Expected '{}' to exist after build.\nMissing: {}",
            llvm_config,
            built_llvm_config.display()
        ));
    }

    fs::copy(&built_llvm_config, ws.bundle_bin_dir.join(llvm_config))
        .with_context(|| "llvm-config should be copied into bundle bin dir")?;

    endgroup!();
    Ok(())
}

fn cleanup_bundle(ws: &BundleWorkspace) -> anyhow::Result<()> {
    let install_dir = &ws.bundle_install_dir;
    let libdir = &ws.bundle_lib_dir;

    let _ = fs::remove_dir_all(install_dir.join("libexec"));
    let _ = fs::remove_dir_all(install_dir.join("share"));
    let _ = fs::remove_dir_all(libdir.join("libscanbuild"));
    let _ = fs::remove_dir_all(libdir.join("libear"));
    let _ = fs::remove_dir_all(libdir.join("objects-Release"));

    // Remove known extras static libs
    let drop_static = [
        "libmlir_c_runner_utils",
        "libmlir_runner_utils",
        "libmlir_async_runtime",
        "libmlir_arm_runner_utils",
        "libmlir_float16_utils",
        "libmlir_arm_sme_abi_stubs",
    ];
    for base in drop_static {
        let _ = fs::remove_file(libdir.join(format!("{base}.a")));
        let _ = fs::remove_file(libdir.join(format!("{base}.lib")));
    }

    // Remove libLTO and libRemarks shared variants
    let sh_ext = if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(windows) {
        "dll"
    } else {
        "so"
    };

    for base in ["libLTO", "libRemarks"] {
        let _ = fs::remove_file(libdir.join(format!("{base}.{sh_ext}")));
        // Remove versioned suffixes on unix if present: libLTO.so.* etc.
        if let Ok(entries) = fs::read_dir(libdir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with(&format!("{base}.{sh_ext}.")) {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }

    Ok(())
}

fn repo_root() -> anyhow::Result<PathBuf> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .with_context(|| "git rev-parse should run")?;

    if !out.status.success() {
        return Err(anyhow::anyhow!("git rev-parse should succeed"));
    }

    let s = String::from_utf8(out.stdout)?;
    Ok(PathBuf::from(s.trim()))
}

fn chrono_utc_iso8601() -> String {
    let now = std::time::SystemTime::now();
    let dt: chrono::DateTime<chrono::Utc> = now.into();
    dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn clean_bundle_workspace(workspace_dir: &Path, yes: bool) -> anyhow::Result<()> {
    let ws = workspace_dir.to_path_buf();
    let ws = ws.canonicalize().unwrap_or(ws);

    if !ws.exists() {
        println!("Workspace '{}' does not exist", ws.display());
        return Ok(());
    }

    if !yes {
        print!(
            "This will permanently delete workpace '{}'. Continue? [y/N]: ",
            ws.display()
        );
        io::stdout().flush().expect("stdout should flush");

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim();
        if !matches!(input, "y" | "Y" | "yes" | "YES") {
            println!("Aborted.");
            return Ok(());
        }
    }

    println!("Deleting workspace '{}'...", ws.display());
    fs::remove_dir_all(&ws).with_context(|| "workspace directory should be deleted")?;
    println!("Workspace deleted.");

    Ok(())
}
