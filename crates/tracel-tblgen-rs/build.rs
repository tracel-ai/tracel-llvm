use std::{env, error::Error, ffi::OsString, process::exit};

fn main() {
    // in xtask mode we skip the bundler installation alltogether
    if std::env::var_os("CARGO_FEATURE_XTASK").is_some() {
        println!("cargo:warning=xtask mode enabled, skipping bundle installation.");
        return;
    }

    if let Err(err) = run() {
        eprintln!("{err}");
        exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let llvm_major = tracel_llvm_bundler::config::init()?;
    let prefix_var = format!("TABLEGEN_{llvm_major}0_PREFIX");
    println!("cargo:rerun-if-env-changed={prefix_var}");
    let prefix: Option<OsString> = env::var_os(&prefix_var);
    // Double check the version
    let version = tracel_llvm_bundler::config::get_version(prefix.as_ref())?;
    if !version.starts_with(&format!("{llvm_major}.")) {
        return Err(format!(
            "llvm-config version should be {llvm_major}.x.x (found {version})"
        ).into());
    }
    // Search paths
    let libdir = tracel_llvm_bundler::config::get_libdir(prefix.as_ref())?;
    println!("cargo:rustc-link-search=native={libdir}");
    // Link configuration
    for lib in tracel_llvm_bundler::config::get_libs(prefix.as_ref())? {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    for sys in tracel_llvm_bundler::config::get_system_libs(prefix.as_ref())? {
        println!("cargo:rustc-link-lib={sys}");
    }
    if let Some(cpp) = tracel_llvm_bundler::config::get_system_libcpp() {
        println!("cargo:rustc-link-lib={cpp}");
    }
    // Link tblgen shim
    println!("cargo:rustc-link-lib=static=CTableGen");
    tracel_llvm_bundler::config::set_homebrew_library_path()?;
    Ok(())
}
