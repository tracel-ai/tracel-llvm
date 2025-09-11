use std::{ffi::OsString, path::{Path, PathBuf}, process::Command};

use crate::error::{BundlerResult, BundlingError};

/// Returns a vector of all libraries required by LLVM.
pub fn get_libs(prefix_os: Option<&OsString>) -> BundlerResult<Vec<String>> {
    let libs = llvm_config(prefix_os, "--libs")?;
    Ok(libs.trim().strip_prefix("-l").unwrap().split(" -l").map(str::to_owned).collect())
}

/// Returns a vector of all library names required by LLVM.
pub fn get_libnames(prefix_os: Option<&OsString>) -> BundlerResult<Vec<String>> {
    let libs = llvm_config(prefix_os, "--libnames")?;
    Ok(libs.trim().split(" ").map(str::to_owned).collect())
}

/// Returns a vector of all system libraries required by LLVM.
pub fn get_system_libs(prefix_os: Option<&OsString>) -> BundlerResult<Vec<String>> {
    let libs = llvm_config(prefix_os, "--system-libs")?;
    Ok(libs.trim().strip_prefix("-l").unwrap().split(" -l").map(str::to_owned).collect())
}

/// Returns the lib directory path
pub fn get_libdir(prefix_os: Option<&OsString>) -> BundlerResult<String> {
    let libdir = llvm_config(prefix_os, "--libdir")?;
    Ok(libdir)
}

/// Returns the includes directory path.
pub fn get_includedir(prefix_os: Option<&OsString>) -> BundlerResult<String> {
    let includedir = llvm_config(prefix_os, "--includedir")?;
    Ok(includedir)
}

/// Returns the LLVM version.
pub fn get_version(prefix_os: Option<&OsString>) -> BundlerResult<String> {
    let version = llvm_config(prefix_os, "--version")?;
    Ok(version)
}

/// Returns the CXX flags
pub fn get_cxxflags(prefix_os: Option<&OsString>) -> BundlerResult<String> {
    let flags = llvm_config(prefix_os, "--cxxflags")?;
    Ok(flags)
}

/// Returns the C flags with some tweaks for portability
pub fn get_cflags(prefix_os: Option<&OsString>) -> BundlerResult<String> {
    let flags = llvm_config(prefix_os, "--cflags")?;
    Ok(flags)
}

pub fn get_system_libcpp() -> Option<&'static str> {
    if cfg!(target_env = "msvc") {
        None
    } else if cfg!(target_os = "macos") {
        Some("c++")
    } else {
        Some("stdc++")
    }
}

/// Run `llvm-config` with a given argument, meant to be used in `build.rs` files.
/// - `prefix_os`: Optional install prefix (usually from an env var).
///   If `None`, we fall back to searching `llvm-config` in PATH.
/// - `argument`: Argument to pass to `llvm-config` (e.g., `--libs`, `--cflags`).
fn llvm_config(prefix_os: Option<&OsString>, argument: &str) -> BundlerResult<String> {
    let prefix = prefix_os
        .as_ref()
        .map(|p| Path::new(p).join("bin"))
        .unwrap_or_default();

    let llvm_config_binary = if cfg!(target_os = "windows") {
        "llvm-config.exe"
    } else {
        "llvm-config"
    };

    // If prefix is empty, fall back to PATH
    let path: PathBuf = if prefix.as_os_str().is_empty() {
        PathBuf::from(llvm_config_binary)
    } else {
        prefix.join(llvm_config_binary)
    };

    // Run llvm-config
    let output = Command::new(&path)
        .arg("--link-static")
        .arg(argument)
        .output()
        .map_err(|e| {
            println!("cargo:warning=Failed to execute `{}` ({})", path.display(), e);

            match e.kind() {
                std::io::ErrorKind::NotFound => {
                    println!("cargo:warning=The computed llvm-config path does not exist or is not executable.");

                    if let Some(val) = prefix_os.as_ref() {
                        println!(
                            "cargo:warning=Computed from prefix='{}' → '{}/bin/{}'",
                            Path::new(val).display(),
                            Path::new(val).display(),
                            llvm_config_binary
                        );
                    } else {
                        println!("cargo:warning=No prefix provided; relying on PATH to find `{llvm_config_binary}`");
                    }

                    println!("cargo:warning=Fixes:");
                    println!("cargo:warning=- Pass the LLVM/MLIR install prefix (the dir that contains `bin/{llvm_config_binary}`)");
                    println!("cargo:warning=- Or ensure `{llvm_config_binary}` is available in PATH");
                }
                _ => {
                    println!(
                        "cargo:warning=I/O error while launching `{}`: {}",
                        path.display(),
                        e
                    );
                }
            }

            BundlingError::IoError(e)
        })?;

    if !output.status.success() {
        let stderr = str::from_utf8(&output.stderr)?.trim().to_owned();
        if !stderr.is_empty() {
            println!("cargo:warning=llvm-config stderr: {stderr}");
        }
        let code = output.status.code().unwrap_or(-1);
        return Err(BundlingError::ToolExit {
            path: path.display().to_string(),
            status: code,
            stderr,
        });
    }

    let stdout = output.stdout;
    Ok(str::from_utf8(&stdout)?.trim().to_string())
}
