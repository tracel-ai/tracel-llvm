use std::{env, error::Error, ffi::OsString};

const LLVM_MAJOR_VERSION: usize = 20;

fn main() -> Result<(), Box<dyn Error>> {
    let prefix_env_var = format!("MLIR_SYS_{LLVM_MAJOR_VERSION}0_PREFIX");
    println!("cargo:rerun-if-env-changed={prefix_env_var}");
    tracel_llvm_bundler_rs::bundler::bundle_cache()?;
    let prefix_os: Option<OsString> = env::var_os(prefix_env_var);
    let includedir = tracel_llvm_bundler_rs::config::get_includedir(prefix_os.as_ref())?;
    println!("cargo:rustc-env=LLVM_INCLUDE_DIRECTORY={includedir}");
    // required on macos
    tracel_llvm_bundler_rs::config::set_homebrew_library_path()?;
    Ok(())
}
