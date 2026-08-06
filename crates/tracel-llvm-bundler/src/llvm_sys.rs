//! Build-script support for linking against the bundled LLVM without letting
//! the `llvm-sys` crate drive the discovery.
//!
//! `llvm-sys` resolves LLVM from `LLVM_SYS_<major>_PREFIX` (or `llvm-config` on
//! `PATH`) inside its own build script, which runs *before* the build script of
//! any crate that could have downloaded a bundle first. Cargo gives no way to
//! push an environment variable back up into an already-scheduled build script,
//! so the only way to feed it the bundle would be to fork it.
//!
//! Instead, `llvm-sys` is built with `no-llvm-linking` and
//! `disable-alltargets-init`, which makes its build script return immediately
//! without ever looking for `llvm-config`, and the final consumer calls
//! [`link`] from its own build script to emit everything `llvm-sys` would
//! normally have emitted: the link search path, the LLVM libraries in
//! dependency order, the system libraries, and the target initialization
//! wrappers.

use std::{
    ffi::OsString,
    fs,
    io::{Error as IoError, ErrorKind},
    path::PathBuf,
};

use crate::config::{
    ConfigError, ConfigResult, get_includedir, get_libdir, get_libs, get_system_libcpp,
    get_system_libs, llvm_path, set_homebrew_library_path,
};

/// Wrappers for the `static inline` target initializers of `llvm-c/Target.h`.
///
/// Embedded rather than compiled from disk because the caller is a build script
/// of another crate: its `CARGO_MANIFEST_DIR` does not point here.
const TARGET_WRAPPERS: &str = include_str!("../wrappers/target.c");

/// Emits the link configuration for the bundled LLVM.
///
/// Must be called from the `build.rs` of the crate that ends up linking LLVM,
/// with `tracel-llvm-bundler` as a build dependency so that the bundle is
/// already downloaded by the time this runs.
pub fn link() -> ConfigResult<()> {
    let prefix: OsString = llvm_path()?.into_os_string();

    // The wrappers reference symbols provided by the LLVM archives, so they have
    // to be emitted first to end up earlier on the link line.
    compile_target_wrappers(&prefix)?;

    println!(
        "cargo:rustc-link-search=native={}",
        get_libdir(Some(&prefix))?
    );
    let _ = set_homebrew_library_path();

    // `llvm-config --link-static --libs` already returns the libraries in
    // dependency order.
    for library in get_libs(Some(&prefix))? {
        println!("cargo:rustc-link-lib=static={library}");
    }

    for library in get_system_libs(Some(&prefix))? {
        println!("cargo:rustc-link-lib=dylib={library}");
    }

    // LLVM is C++, so whatever it was built against has to come along.
    if let Some(libcpp) = get_system_libcpp() {
        println!("cargo:rustc-link-lib=dylib={libcpp}");
    }

    Ok(())
}

fn compile_target_wrappers(prefix: &OsString) -> ConfigResult<()> {
    let out_dir = std::env::var_os("OUT_DIR").ok_or_else(|| {
        ConfigError::IoError(IoError::new(
            ErrorKind::NotFound,
            "OUT_DIR is not set, `link` must be called from a build script",
        ))
    })?;

    let source = PathBuf::from(out_dir).join("tracel_llvm_target_wrappers.c");
    fs::write(&source, TARGET_WRAPPERS)?;

    cc::Build::new()
        .file(&source)
        .include(get_includedir(Some(prefix))?)
        .compile("tracel_llvm_target_wrappers");

    Ok(())
}
