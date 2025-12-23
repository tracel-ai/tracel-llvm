use std::path::Path;
use tracel_xtask::{prelude::*, utils::workspace::WorkspaceMember};

use crate::commands::{bindgen::{
    get_bindings_file_path, get_wrapper_file_path,
    llvm_major_version, update_feature_gated_region,
}, BundleWorkspace};

pub fn bindgen(member: &WorkspaceMember, ws: &BundleWorkspace) -> anyhow::Result<()> {
    let major = llvm_major_version()?;
    let prefix_os = std::env::var_os(format!("TABLEGEN_{major}0_PREFIX"));
    let version = tracel_llvm_bundler::config::get_version(prefix_os.as_ref())?;
    if !version.starts_with(&format!("{major}.")) {
        return Err(anyhow::anyhow!(
            "llvm-config version should be {major}.x.x (found {version})"
        ));
    }

    let cc_include = Path::new(&member.path).join("cc").join("include");

    let mut clang_args = vec![
        "-I".to_string(),
        ws.bundle_include_dir.to_string_lossy().into_owned(),
        "-I".to_string(),
        cc_include.to_string_lossy().into_owned(),
    ];
    if cfg!(not(target_os = "windows")) {
        clang_args.extend(["-I".to_string(), "/usr/include".to_string()]);
    }

    group_info!("Generate bindings: {}", member.name);
    let header_path = get_wrapper_file_path(member)?;
    let bindings_path = get_bindings_file_path(member)?;

    bindgen::Builder::default()
        .header(header_path)
        .clang_args(&clang_args)
        .default_enum_style(bindgen::EnumVariation::ModuleConsts)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Bindings generation should succeed")
        .write_to_file(&bindings_path)
        .expect("Bindings file write should succeed");

    update_feature_gated_region(member)?;
    endgroup!();
    Ok(())
}
