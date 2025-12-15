use std::ffi::OsString;

use tracel_xtask::{prelude::*, utils::workspace::WorkspaceMember};

use crate::commands::bindgen::{
    get_bindings_file_path, get_wrapper_file_path, update_feature_gated_region,
};

pub fn bindgen(member: &WorkspaceMember) -> anyhow::Result<()> {
    let llvm_major_version = tracel_llvm_bundler::config::init()?;

    // Install prefix
    let prefix_os: Option<OsString> =
        std::env::var_os(format!("TABLEGEN_{llvm_major_version}0_PREFIX"));

    // Version gate
    let version = tracel_llvm_bundler::config::get_version(prefix_os.as_ref())?;
    if !version.starts_with(&format!("{llvm_major_version}.")) {
        return Err(anyhow::anyhow!(format!(
            "llvm-config should be {llvm_major_version}.x.x (found {version})"
        )));
    }

    let includedir = tracel_llvm_bundler::config::get_includedir(prefix_os.as_ref())?;
    let libdir = tracel_llvm_bundler::config::get_libdir(prefix_os.as_ref())?;

    // clang built-in headers (resource dir)
    let clang_resource_dir = format!("{libdir}/clang/{llvm_major_version}");
    let linux_clang_includedir = format!("{clang_resource_dir}/include");

    let mut clang_args = vec!["-I", &includedir, "-I", "cc/include"];
    if cfg!(not(target_os = "windows")) {
        clang_args.extend(["-I", "/usr/include"]);
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
            "-I",
            &linux_clang_includedir,
            "-resource-dir",
            &clang_resource_dir,
        ]);
    }

    let header_path = get_wrapper_file_path(member)?;
    let bindings_path = get_bindings_file_path(member)?;

    group_info!("Generate bindings: {}", member.name);
    println!("bindings path: {bindings_path}");
    bindgen::Builder::default()
        .header(header_path)
        .clang_args(&clang_args)
        .default_enum_style(bindgen::EnumVariation::ModuleConsts)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Bindings should be generated")
        .write_to_file(&bindings_path)
        .expect("Bindings file should be written");
    update_feature_gated_region(member)?;
    endgroup!();

    Ok(())
}
