use std::{env, ffi::OsString, path::Path};

fn main() {
    let llvm_major_version =
        tracel_llvm_bundler::config::init().expect("Unable to init tracel llvm bundler");
    println!("cargo::rerun-if-changed=build.rs");
    let llvm_version = tracel_llvm_bundler::config::llvm_version();
    println!("cargo:rustc-cfg=feature=\"llvm_{llvm_version}\"");
    // Install prefix
    let prefix_os: Option<OsString> = env::var_os(format!("MLIR_SYS_{llvm_major_version}0_PREFIX"));
    let lib_path = tracel_llvm_bundler::config::get_libdir(prefix_os.as_ref())
        .expect("Unable to get LLVM library directory");
    println!("cargo::rustc-link-search=native={lib_path}");

    let libdir =
        tracel_llvm_bundler::config::get_libdir(prefix_os.as_ref()).expect("Unable to get libdir");
    println!("cargo:rustc-link-search=native={libdir}");
    for lib in
        tracel_llvm_bundler::config::get_libs(prefix_os.as_ref()).expect("Unable to get libs")
    {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    for syslib in tracel_llvm_bundler::config::get_system_libs(prefix_os.as_ref())
        .expect("Unable to get system libs")
    {
        println!("cargo:rustc-link-lib={syslib}");
    }
    if let Some(name) = tracel_llvm_bundler::config::get_system_libcpp() {
        println!("cargo:rustc-link-lib={name}");
    }
    // required on macos
    tracel_llvm_bundler::config::set_homebrew_library_path().expect("Unable to set up homebrew");

    link_mlir_statically(llvm_major_version);
}

fn link_mlir_statically(llvm_major_version: usize) {
    use tracel_llvm_bundler::{
        dependency_graph::DependencyGraph, topological_sort::TopologicalSort,
    };

    let prefix = Path::new(
        &env::var(format!("MLIR_SYS_{llvm_major_version}0_PREFIX"))
            .expect("Not found env variable"),
    )
    .join("lib")
    .join("cmake")
    .join("mlir")
    .join("MLIRTargets.cmake");
    let path = DependencyGraph::from_cmake(prefix).expect("Path not found");
    let mlirlib = TopologicalSort::get_ordered_list(&path);

    for lib in mlirlib.iter().rev() {
        println!("cargo:rustc-link-lib=static={lib}");
    }
}
