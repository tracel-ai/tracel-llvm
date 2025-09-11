use std::{env, error::Error, ffi::OsString, path::Path, process::exit};

const LLVM_MAJOR_VERSION: usize = 20;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        exit(1);
    }
}

fn link_mlir_statically() -> Result<(), Box<dyn Error>> {
    use tracel_llvm_bundler_rs::{
        dependency_graph::DependencyGraph, topological_sort::TopologicalSort,
    };

    let prefix = Path::new(&env::var(format!("MLIR_SYS_{LLVM_MAJOR_VERSION}0_PREFIX"))?)
        .join("lib")
        .join("cmake")
        .join("mlir")
        .join("MLIRTargets.cmake");
    let path = DependencyGraph::from_cmake(prefix)?;
    let mlirlib = TopologicalSort::get_ordered_list(&path);

    for lib in mlirlib.iter().rev() {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    Ok(())
}

fn run() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=wrapper.h");
    // Build cache
    tracel_llvm_bundler_rs::bundler::bundle_cache()?;
    // Install prefix
    let prefix_os: Option<OsString> = env::var_os(format!("MLIR_SYS_{LLVM_MAJOR_VERSION}0_PREFIX"));
    // Version gate
    let version = tracel_llvm_bundler_rs::config::get_version(prefix_os.as_ref())?;
    if !version.starts_with(&format!("{LLVM_MAJOR_VERSION}.")) {
        return Err(format!(
            "failed to find correct version ({LLVM_MAJOR_VERSION}.x.x) of llvm-config (found {version})"
        )
        .into());
    }
    // Libraries and headers
    let includedir = tracel_llvm_bundler_rs::config::get_includedir(prefix_os.as_ref())?;
    let libdir = tracel_llvm_bundler_rs::config::get_libdir(prefix_os.as_ref())?;
    println!("cargo:rustc-link-search=native={libdir}");
    for lib in tracel_llvm_bundler_rs::config::get_libs(prefix_os.as_ref())? {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    for syslib in tracel_llvm_bundler_rs::config::get_system_libs(prefix_os.as_ref())? {
        println!("cargo:rustc-link-lib={syslib}");
    }
    if let Some(name) = tracel_llvm_bundler_rs::config::get_system_libcpp() {
        println!("cargo:rustc-link-lib={name}");
    }
    // required on macos
    tracel_llvm_bundler_rs::config::set_homebrew_library_path()?;

    link_mlir_statically()?;

    bindgen::builder()
        .header("wrapper.h")
        .clang_args(["-I", &includedir])
        .clang_args(["-I", "/usr/include"])
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .unwrap()
        .write_to_file(Path::new(&env::var("OUT_DIR")?).join("bindings.rs"))?;

    Ok(())
}
