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
    ConfigError, ConfigResult, get_includedir, get_libdir, get_libs, get_shared_libs,
    get_shared_system_libs, get_system_libcpp, get_system_libs, llvm_path,
    set_homebrew_library_path,
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
    // `TRACEL_LLVM_SYSTEM_PREFIX` opts out of the downloaded bundle and links a system
    // LLVM instead. Useful when the bundle lacks the host target (e.g. AArch64) or when
    // the system package is shared-only. The prefix must contain `bin/llvm-config`.
    let system_prefix = std::env::var_os("TRACEL_LLVM_SYSTEM_PREFIX");
    let prefix: OsString = match &system_prefix {
        Some(p) => p.clone(),
        None => llvm_path()?.into_os_string(),
    };

    compile_target_wrappers(&prefix)?;

    let libdir = get_libdir(Some(&prefix))?;
    println!("cargo:rustc-link-search=native={libdir}");
    let _ = set_homebrew_library_path();

    if system_prefix.is_some() {
        // Shared libraries must come after the objects on the link line, so use
        // `rustc-link-arg` rather than `rustc-link-lib` here.
        println!("cargo:rustc-link-arg=-Wl,-rpath,{libdir}");
        for library in get_shared_libs(Some(&prefix))? {
            // `rustc-link-lib` is propagated to downstream crates; `rustc-link-arg` is not.
            println!("cargo:rustc-link-lib=dylib={library}");
        }
    } else {
        // `llvm-config --link-static --libs` already returns the libraries in
        // dependency order.
        for library in get_libs(Some(&prefix))? {
            println!("cargo:rustc-link-lib=static={library}");
        }
    }

    if system_prefix.is_some() {
        for library in get_shared_system_libs(Some(&prefix))? {
            println!("cargo:rustc-link-lib=dylib={library}");
        }
    } else {
        for library in get_system_libs(Some(&prefix))? {
            println!("cargo:rustc-link-lib=dylib={library}");
        }
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

    let source = PathBuf::from(&out_dir).join("tracel_llvm_target_wrappers.c");
    fs::write(&source, TARGET_WRAPPERS)?;

    cc::Build::new()
        .file(&source)
        .include(get_includedir(Some(prefix))?)
        .opt_level(3)
        .cargo_metadata(false) // we emit the link directive ourselves, with whole-archive
        .compile("tracel_llvm_target_wrappers");

    // `cc` emits its link directives from this crate's build script, i.e. a build
    // dependency. Those do not reach the final binary, so re-emit them from `link()`,
    // which runs in the consuming crate's build script.
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    // The archive is emitted early on the link line, before crates that reference these
    // initializers, so include it whole rather than relying on archive member selection.
    println!("cargo:rustc-link-lib=static:+whole-archive=tracel_llvm_target_wrappers");

    Ok(())
}
