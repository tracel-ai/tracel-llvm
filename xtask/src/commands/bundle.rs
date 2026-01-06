use std::{
    collections::HashMap,
    fs,
    io::{self, Write as _},
    path::{Path, PathBuf},
};

use clap::{Args, Subcommand};
use tracel_llvm_bundler::build_support::checksums::{sha256_file_hex, sha256_tree_content_hex};
use tracel_xtask::prelude::{anyhow::Context as _, *};

use crate::utils::tblgen_shim::{CTableGenShimConfig, build_and_install_ctablegen_shim};

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
    if cfg!(windows) {
        crate::utils::process::require_tools(&["git", "cmake", "ninja", "tar", "7z"])?;
    } else {
        crate::utils::process::require_tools(&["git", "cmake", "ninja", "tar"])?;
    }

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
            fs::remove_dir_all(dir)
                .with_context(|| format!("Should delete '{}'", dir.display()))?;
        }
    }
    ws.clone_llvm_project_fresh()?;
    ws.build_mlir_project()?;
    // Keep only llvm-config in bin directory.
    prune_bin_to_llvm_config(ws)?;
    // Build  CTableGen shim into the bundle lib directory.
    group_info!("Bundle: build CTableGen shim");
    let shim_cfg = CTableGenShimConfig {
        repo_root: git::git_repo_root_or_cwd()?,
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

        eprintln!("Creating archive, please wait...");
        create_tar_xz(&out_archive, &ws, &pkg_dir_name)?;

        eprintln!("Computing checksums, please wait...");
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
        fs::write(&sidecar, serde_json::to_vec_pretty(&manifest)?)
            .with_context(|| "checksums sidecar should be written")?;

        group_info!("Bundle outputs");
        println!("Install dir: {}", ws.bundle_install_dir.display());
        println!("Archive:     {}", out_archive.display());
        println!("Sidecar:     {}", sidecar.display());
        endgroup!();
    }
    endgroup!();

    Ok(())
}

/// Creates a `.tar.xz` using CLI tools
fn create_tar_xz(
    out_archive: &Path,
    workspace: &BundleWorkspace,
    top_level_name: &str,
) -> anyhow::Result<()> {
    let tar_name = Path::new(
        Path::new(&out_archive)
            .file_stem()
            .expect("archive path should have a file name"),
    );
    let archive_name = Path::new(
        Path::new(&out_archive)
            .file_name()
            .expect("archive path should have a file name"),
    );

    if out_archive.exists() {
        fs::remove_file(&out_archive).with_context(|| "archive file should be removed")?;
    }
    if tar_name.exists() {
        fs::remove_file(tar_name)?;
    }

    let archive_name = archive_name.to_string_lossy();
    let tar_name = tar_name.to_string_lossy();
    if cfg!(windows) {
        create_tar_xz_windows(
            &archive_name,
            &tar_name,
            &workspace.workspace_dir,
            top_level_name,
        )?;
    } else {
        create_tar_xz_unix(&archive_name, &workspace.workspace_dir, top_level_name)?;
    }

    Ok(())
}

fn create_tar_xz_windows(
    archive_name: &str,
    tar_name: &str,
    workdir: &Path,
    top_level_name: &str,
) -> anyhow::Result<()> {
    run_process(
        "tar",
        &["-cf", &tar_name, top_level_name],
        None,
        Some(workdir),
        "",
    )?;
    run_process(
        "7z",
        &[
            "a",
            "-txz",
            "-mx=9",
            "-m0=lzma2",
            "-md=1536m",
            "-mfb=273",
            "-mmt=on",
            &archive_name,
            &tar_name,
        ],
        None,
        Some(workdir),
        "",
    )?;
    Ok(())
}

fn create_tar_xz_unix(
    archive_name: &str,
    workdir: &Path,
    top_level_name: &str,
) -> anyhow::Result<()> {
    if cfg!(target_os = "macos") {
        let envs: HashMap<&str, &str> = [("COPYFILE_DISABLE", "1")].into_iter().collect();
        run_process(
            "tar",
            &[
                "--no-mac-metadata",
                "--no-xattrs",
                "-cJf",
                archive_name,
                top_level_name,
            ],
            Some(envs),
            Some(workdir),
            "",
        )?;
    } else {
        // Linux
        run_process(
            "tar",
            &["-cJf", archive_name, top_level_name],
            None,
            Some(workdir),
            "",
        )?;
    }

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
