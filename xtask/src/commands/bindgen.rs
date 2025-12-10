use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
};

use anyhow::anyhow;
use tracel_xtask::{
    prelude::*,
    utils::workspace::{WorkspaceMember, WorkspaceMemberType, get_workspace_members},
};

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
            let bindings_path =
                get_bindings_file_path(&member, &tracel_llvm_bundler::config::llvm_version())?;
            println!("bindings path: {bindings_path}");
            // Generate bindings using bindgen
            bindgen::Builder::default()
                .header(header_path)
                .clang_args(&clang_args)
                .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
                .generate()
                .expect("Should generate LLVM bindings")
                .write_to_file(bindings_path)
                .expect("Should write bindings file");
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

fn get_bindings_file_path(member: &WorkspaceMember, patch: &str) -> anyhow::Result<String> {
    let out_path = get_output_path(member)?;
    let path = out_path.join(format!("bindings_{patch}.rs"));
    Ok(path.to_string_lossy().into_owned())
}

fn get_wrapper_file_path(member: &WorkspaceMember) -> anyhow::Result<String> {
    let out_path = get_input_path(member)?;
    let path = out_path.join("wrapper.h");
    Ok(path.to_string_lossy().into_owned())
}
