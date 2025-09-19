use std::{env, error::Error, ffi::OsString};

fn main() -> Result<(), Box<dyn Error>> {
    let llvm_major_version = tracel_llvm_bundler::config::init()?;
    let prefix_env_var = format!("MLIR_SYS_{llvm_major_version}0_PREFIX");
    println!("cargo:rerun-if-env-changed={prefix_env_var}");
    let prefix_os: Option<OsString> = env::var_os(prefix_env_var);
    let includedir = tracel_llvm_bundler::config::get_includedir(prefix_os.as_ref())?;
    println!("cargo:rustc-env=LLVM_INCLUDE_DIRECTORY={includedir}");
    // required on macos
    tracel_llvm_bundler::config::set_homebrew_library_path()?;
    Ok(())
}
