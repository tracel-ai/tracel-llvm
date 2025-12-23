use tracel_xtask::{prelude::*, utils::workspace::WorkspaceMember};

use crate::commands::{
    bindgen::{get_bindings_file_path, get_wrapper_file_path, update_feature_gated_region},
    BundleWorkspace,
};

pub(crate) fn bindgen(member: &WorkspaceMember, ws: &BundleWorkspace) -> anyhow::Result<()> {
    let major = crate::commands::bindgen::llvm_major_version()?;
    let prefix_os = std::env::var_os(format!("MLIR_SYS_{major}0_PREFIX"));
    let version = tracel_llvm_bundler::config::get_version(prefix_os.as_ref())?;
    if !version.starts_with(&format!("{major}.")) {
        return Err(anyhow::anyhow!(
            "llvm-config version should be {major}.x.x (found {version})"
        ));
    }

    let clang_args = vec![
        "-I".to_string(),
        ws.bundle_include_dir.to_string_lossy().into_owned(),
        "-I".to_string(),
        ws.clang_include_dir.to_string_lossy().into_owned(),
        "-I".to_string(),
        ws.clang_resource_include_dir.to_string_lossy().into_owned(),
        "-x".to_string(),
        "c++".to_string(),
        "-std=c++17".to_string(),
        format!("-resource-dir={}", ws.clang_resource_dir.to_string_lossy()),
    ];

    group_info!("Generate bindings: {}", member.name);
    let header_path = get_wrapper_file_path(member)?;
    let bindings_path = get_bindings_file_path(member)?;

    bindgen::Builder::default()
        .header(header_path)
        .clang_args(&clang_args)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .derive_debug(true)
        .layout_tests(false)
        .derive_default(true)
        .raw_line("#![allow(non_camel_case_types)]")
        .raw_line("#![allow(non_snake_case)]")
        .raw_line("#![allow(non_upper_case_globals)]")
        .raw_line("#![allow(dead_code)]")
        .generate()
        .expect("Bindings generation should succeed")
        .write_to_file(&bindings_path)
        .expect("Bindings file write should succeed");

    update_feature_gated_region(member)?;
    endgroup!();
    Ok(())
}
