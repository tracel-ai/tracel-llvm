use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

use anyhow::anyhow;
use tracel_xtask::{
    prelude::*,
    utils::workspace::{WorkspaceMember, WorkspaceMemberType, get_workspace_members},
};

const FEATURE_GATED_REGION_BEGIN: &str = "// BEGIN AUTO-GENERATED FEATURE GATED REGION";
const FEATURE_GATED_REGION_END: &str = "// END AUTO-GENERATED FEATURE GATED BINDINGS";

#[derive(clap::Args)]
pub struct BindgenCmdArgs {
    /// Name of the crates for which we need to generate bindings. Pass "all" for all crates.
    #[arg(short, long, value_delimiter = ',', default_value = "tracel-mlir-sys")]
    crates: Vec<String>,
}

pub(crate) fn handle_command(args: BindgenCmdArgs) -> anyhow::Result<()> {
    run_bindgen(&args.crates)
}

fn run_bindgen(crates: &[String]) -> anyhow::Result<()> {
    let llvm_major_version = tracel_llvm_bundler::config::init()?;
    println!("cargo:rerun-if-changed=wrapper.h");
    // Install prefix
    let prefix_os: Option<OsString> = env::var_os(format!("MLIR_SYS_{llvm_major_version}0_PREFIX"));
    // Version gate
    let version = tracel_llvm_bundler::config::get_version(prefix_os.as_ref())?;
    if !version.starts_with(&format!("{llvm_major_version}.")) {
        return Err(anyhow!(format!(
            "failed to find correct version ({llvm_major_version}.x.x) of llvm-config (found {version})"
        )));
    }
    // Libraries and headers
    let includedir = tracel_llvm_bundler::config::get_includedir(prefix_os.as_ref())?;
    let libdir = tracel_llvm_bundler::config::get_libdir(prefix_os.as_ref())?;
    // clang built-in headers (resource dir)
    let clang_resource_dir = format!("{libdir}/clang/{llvm_major_version}");
    let linux_clang_includedir = format!("{clang_resource_dir}/include");

    let mut clang_args = vec!["-I", &includedir];
    if cfg!(not(target_os = "windows")) {
        clang_args.extend(vec!["-I", "/usr/include"]);
    }
    if cfg!(target_os = "linux") {
        // bindgen (clang-sys) will dlopen libclang from here
        unsafe {
            std::env::set_var("LIBCLANG_PATH", &libdir);

            // make absolutely sure our libdir is searched first by the loader
            match std::env::var("LD_LIBRARY_PATH") {
                Ok(old) => std::env::set_var("LD_LIBRARY_PATH", format!("{libdir}:{old}")),
                Err(_) => std::env::set_var("LD_LIBRARY_PATH", &libdir),
            }
        }
        clang_args.extend([
            "-I",
            &linux_clang_includedir,
            "-resource-dir",
            &clang_resource_dir, // key to avoid picking system headers
        ]);
    }

    let members = get_workspace_members(WorkspaceMemberType::Crate);
    for member in members {
        if member.name == "all" || crates.contains(&member.name) {
            group_info!("Generate bindings: {}", member.name);

            let header_path = get_wrapper_file_path(&member)?;
            let bindings_path = get_bindings_file_path(&member)?;
            println!("bindings path: {bindings_path}");

            bindgen::Builder::default()
                .header(header_path)
                .clang_args(&clang_args)
                .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
                .generate()
                .expect("Should generate LLVM bindings")
                .write_to_file(&bindings_path)
                .expect("Should write bindings file");
            update_feature_gated_region(&member)?;

            endgroup!();
        } else {
            group_info!("Skip '{}' because it has been excluded!", &member.name);
        }
    }

    Ok(())
}

fn get_output_path(member: &WorkspaceMember) -> anyhow::Result<PathBuf> {
    let path = Path::new(&member.path).join("src").join("bindings");
    if path.exists() {
        Ok(path)
    } else {
        Err(anyhow::anyhow!(
            "Cannot find output path: {}",
            path.display()
        ))
    }
}

fn get_input_path(member: &WorkspaceMember) -> anyhow::Result<PathBuf> {
    let path = Path::new(&member.path).to_path_buf();
    if path.exists() {
        Ok(path)
    } else {
        Err(anyhow::anyhow!(
            "Cannot find input path: {}",
            path.display()
        ))
    }
}

fn platform_suffix_for_feature() -> String {
    // Example: "linux_x86_64", "macos_aarch64"
    format!("{}_{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn sanitize_for_ident(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn get_bindings_file_path(member: &WorkspaceMember) -> anyhow::Result<String> {
    let out_path = get_output_path(member)?;
    let platform = platform_suffix_for_feature();
    // Example: bindings_macos_aarch64.rs, bindings_linux_x86_64.rs
    let filename = format!("bindings_{}.rs", platform);
    let path = out_path.join(filename);
    Ok(path.to_string_lossy().into_owned())
}

fn get_wrapper_file_path(member: &WorkspaceMember) -> anyhow::Result<String> {
    let out_path = get_input_path(member)?;
    let path = out_path.join("wrapper.h");
    Ok(path.to_string_lossy().into_owned())
}

fn get_selector_file_path(member: &WorkspaceMember) -> anyhow::Result<PathBuf> {
    let out_path = get_output_path(member)?; // .../src/bindings
    Ok(out_path.join("mod.rs"))
}

fn ensure_selector_file(member: &WorkspaceMember) -> anyhow::Result<PathBuf> {
    let selector_path = get_selector_file_path(member)?;

    if !selector_path.exists() {
        let mut content = String::new();
        content.push_str("//! Auto-generated binding selector. Do not edit by hand.\n");
        content.push_str("//! This file is partially managed by xtask.\n");
        content.push_str("\n");
        content.push_str(FEATURE_GATED_REGION_BEGIN);
        content.push('\n');
        content.push_str(FEATURE_GATED_REGION_END);
        content.push('\n');

        fs::write(&selector_path, content).expect("Should create selector file");
    } else {
        // Optional: ensure markers exist; fail loudly if they don't.
        let text = fs::read_to_string(&selector_path).expect("Should read selector file");

        if !text.contains(FEATURE_GATED_REGION_BEGIN) || !text.contains(FEATURE_GATED_REGION_END) {
            return Err(anyhow!(
                "Selector file {} is missing FEATURE GATED REGION markers",
                selector_path.display()
            ));
        }
    }

    Ok(selector_path)
}

fn update_feature_gated_region(member: &WorkspaceMember) -> anyhow::Result<()> {
    let selector_path = ensure_selector_file(member)?;
    let bindings_dir = get_output_path(member)?;

    // Collect bindings_*.rs files
    // entry = (module_name, os, arch)
    let mut entries: Vec<(String, String, String)> = Vec::new();

    for entry in fs::read_dir(&bindings_dir).expect("Should read bindings directory") {
        let entry = entry.expect("Should read directory entry");
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => continue,
        };

        if !name.starts_with("bindings_") || !name.ends_with(".rs") {
            continue;
        }

        // Strip "bindings_" and ".rs"
        let stem = &name["bindings_".len()..name.len() - ".rs".len()];
        // Expected format: "<os>_<arch>", e.g. "macos_aarch64"
        let segments: Vec<&str> = stem.split('_').collect();
        if segments.len() != 2 {
            // Not a "<os>_<arch>" pattern, skip
            continue;
        }

        let os = segments[0].to_string();
        let arch = segments[1].to_string();

        // Module name from filename without ".rs", sanitized
        let module_name = sanitize_for_ident(name.strip_suffix(".rs").unwrap());
        // e.g. "bindings_macos_aarch64"

        entries.push((module_name, os, arch));
    }

    // Stable ordering for idempotence
    entries.sort_by(|a, b| {
        a.1.cmp(&b.1) // os
            .then(a.2.cmp(&b.2)) // arch
    });

    // Build the generated region
    let mut generated = String::new();
    let mut conditions: Vec<String> = Vec::new();

    for (module, os, arch) in &entries {
        let cond = format!("all(target_os = \"{os}\", target_arch = \"{arch}\")");
        conditions.push(cond.clone());

        generated.push_str(&format!("#[cfg({cond})]\n"));
        generated.push_str(&format!("mod {module};\n\n"));

        generated.push_str(&format!("#[cfg({cond})]\n"));
        generated.push_str(&format!("pub use {module}::*;\n\n"));
    }

    if !entries.is_empty() {
        let joined_conditions = conditions
            .iter()
            .map(|c| format!("    {c}"))
            .collect::<Vec<_>>()
            .join(",\n");

        generated.push_str("#[cfg(not(any(\n");
        generated.push_str(&joined_conditions);
        generated.push_str("\n)))]\n");
        generated.push_str(
            "compile_error!(\"No pre-generated MLIR bindings available for this target_os/target_arch combination.\");\n",
        );
    } else {
        generated.push_str(
            "compile_error!(\"No generated bindings modules were found in src/bindings.\");\n",
        );
    }

    // Inject into FEATURE GATED REGION
    let existing = fs::read_to_string(&selector_path).expect("Should read selector file");

    let begin_idx = existing
        .find(FEATURE_GATED_REGION_BEGIN)
        .ok_or_else(|| anyhow!("Should find FEATURE GATED REGION begin marker"))?;
    let end_idx = existing
        .find(FEATURE_GATED_REGION_END)
        .ok_or_else(|| anyhow!("Should find FEATURE GATED REGION end marker"))?;

    let before = &existing[..begin_idx + FEATURE_GATED_REGION_BEGIN.len()];
    let after = &existing[end_idx..];

    let mut new_content = String::new();
    new_content.push_str(before);
    new_content.push('\n');
    new_content.push('\n');
    new_content.push_str(&generated);
    new_content.push('\n');
    new_content.push_str(after);

    if new_content != existing {
        fs::write(&selector_path, new_content).expect("Should update selector file");
    }

    Ok(())
}
