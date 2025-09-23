use std::{env, error::Error, ffi::OsString, path::Path, process::exit};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        exit(1);
    }
}

fn link_mlir_statically(llvm_major_version: usize) -> Result<(), Box<dyn Error>> {
    use tracel_llvm_bundler::{
        dependency_graph::DependencyGraph, topological_sort::TopologicalSort,
    };

    let prefix = Path::new(&env::var(format!("MLIR_SYS_{llvm_major_version}0_PREFIX"))?)
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
    let llvm_major_version = tracel_llvm_bundler::config::init()?;
    println!("cargo:rerun-if-changed=wrapper.h");
    // Install prefix
    let prefix_os: Option<OsString> = env::var_os(format!("MLIR_SYS_{llvm_major_version}0_PREFIX"));
    // Version gate
    let version = tracel_llvm_bundler::config::get_version(prefix_os.as_ref())?;
    if !version.starts_with(&format!("{llvm_major_version}.")) {
        return Err(format!(
            "failed to find correct version ({llvm_major_version}.x.x) of llvm-config (found {version})"
        )
        .into());
    }
    // Libraries and headers
    let includedir = tracel_llvm_bundler::config::get_includedir(prefix_os.as_ref())?;
    let libdir = tracel_llvm_bundler::config::get_libdir(prefix_os.as_ref())?;
    println!("cargo:rustc-link-search=native={libdir}");
    for lib in tracel_llvm_bundler::config::get_libs(prefix_os.as_ref())? {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    for syslib in tracel_llvm_bundler::config::get_system_libs(prefix_os.as_ref())? {
        println!("cargo:rustc-link-lib={syslib}");
    }
    if let Some(name) = tracel_llvm_bundler::config::get_system_libcpp() {
        println!("cargo:rustc-link-lib={name}");
    }
    // required on macos
    tracel_llvm_bundler::config::set_homebrew_library_path()?;

    link_mlir_statically(llvm_major_version)?;

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
                Err(_)  => std::env::set_var("LD_LIBRARY_PATH", &libdir),
            }
        }
        clang_args.extend([
            "-I", &linux_clang_includedir,
            "-resource-dir", &clang_resource_dir, // key to avoid picking system headers
        ]);
    }

    bindgen::builder()
        .header("wrapper.h")
        .clang_args(&clang_args)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .unwrap()
        .write_to_file(Path::new(&env::var("OUT_DIR")?).join("bindings.rs"))?;

    Ok(())
}
