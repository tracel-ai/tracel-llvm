use tracel_xtask::{prelude::*, utils::workspace::WorkspaceMember};

use crate::commands::bindgen::{
    get_bindings_file_path, get_wrapper_file_path, update_feature_gated_region,
};

pub(crate) fn bindgen(member: &WorkspaceMember) -> anyhow::Result<()> {
    use std::{env, ffi::OsString};

    use anyhow::anyhow;
    use tracel_xtask::prelude::*;

    let llvm_major_version = tracel_llvm_bundler::config::init()?;
    println!("cargo:rerun-if-changed=wrapper.h");

    // Install prefix
    let prefix_os: Option<OsString> = env::var_os(format!("MLIR_SYS_{llvm_major_version}0_PREFIX"));

    // Version gate
    let version = tracel_llvm_bundler::config::get_version(prefix_os.as_ref())?;
    if !version.starts_with(&format!("{llvm_major_version}.")) {
        return Err(anyhow!(
            "llvm-config version should be {llvm_major_version}.x.x (found {version})"
        ));
    }

    // Libraries and headers
    let includedir = tracel_llvm_bundler::config::get_includedir(prefix_os.as_ref())?;
    let libdir = tracel_llvm_bundler::config::get_libdir(prefix_os.as_ref())?;

    // clang built-in headers (resource dir)
    let clang_resource_dir = format!("{libdir}/clang/{llvm_major_version}");
    let linux_clang_includedir = format!("{clang_resource_dir}/include");

    let mut clang_args = vec!["-I".to_string(), includedir.clone()];
    if cfg!(not(target_os = "windows")) {
        clang_args.extend(["-I".to_string(), "/usr/include".to_string()]);
    }
    if cfg!(target_os = "linux") {
        unsafe {
            std::env::set_var("LIBCLANG_PATH", &libdir);
            match std::env::var("LD_LIBRARY_PATH") {
                Ok(old) => std::env::set_var("LD_LIBRARY_PATH", format!("{libdir}:{old}")),
                Err(_) => std::env::set_var("LD_LIBRARY_PATH", &libdir),
            }
        }
        clang_args.extend([
            "-I".to_string(),
            linux_clang_includedir,
            "-resource-dir".to_string(),
            clang_resource_dir,
        ]);
    }

    group_info!("Generate bindings: {}", member.name);
    let header_path = get_wrapper_file_path(member)?;
    let bindings_path = get_bindings_file_path(member)?;
    println!("bindings path: {bindings_path}");
    bindgen::Builder::default()
        .header(header_path)
        .clang_args(&clang_args)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Bindings generation should succeed")
        .write_to_file(&bindings_path)
        .expect("Bindings file write should succeed");
    update_feature_gated_region(member)?;
    endgroup!();
    Ok(())
}
