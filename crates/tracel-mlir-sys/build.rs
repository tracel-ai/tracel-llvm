use std::{env, ffi::OsString, path::Path};

fn main() {
    // in xtask mode we skip the bundler installation alltogether
    if std::env::var_os("CARGO_FEATURE_XTASK").is_some() {
        println!("cargo:warning=xtask mode enabled, skipping bundle installation.");
        return;
    }

    let llvm_major_version =
        tracel_llvm_bundler::config::init().expect("Should init tracel llvm bundler");

    println!("cargo:rerun-if-changed=build.rs");

    let prefix_os: Option<OsString> =
        env::var_os(format!("MLIR_SYS_{llvm_major_version}0_PREFIX"));
    let libdir = tracel_llvm_bundler::config::get_libdir(prefix_os.as_ref())
        .expect("Should get LLVM library directory");
    println!("cargo:rustc-link-search=native={libdir}");

    for lib in tracel_llvm_bundler::config::get_libs(prefix_os.as_ref())
        .expect("Should get libs")
    {
        println!("cargo:rustc-link-lib=static={lib}");
    }

    for syslib in tracel_llvm_bundler::config::get_system_libs(prefix_os.as_ref())
        .expect("Should get system libs")
    {
        println!("cargo:rustc-link-lib={syslib}");
    }

    if let Some(name) = tracel_llvm_bundler::config::get_system_libcpp() {
        println!("cargo:rustc-link-lib={name}");
    }

    tracel_llvm_bundler::config::set_homebrew_library_path()
        .expect("Should set up homebrew library path");

    link_mlir_statically(llvm_major_version);
}

fn link_mlir_statically(llvm_major_version: usize) {
    use tracel_llvm_bundler::{
        dependency_graph::DependencyGraph,
        topological_sort::TopologicalSort,
    };

    let prefix = Path::new(
        &env::var(format!("MLIR_SYS_{llvm_major_version}0_PREFIX"))
            .expect("Should find MLIR_SYS prefix env variable"),
    )
    .join("lib")
    .join("cmake")
    .join("mlir")
    .join("MLIRTargets.cmake");

    let graph = DependencyGraph::from_cmake(prefix)
        .expect("Should load MLIRTargets.cmake dependency graph");
    let mlirlib = TopologicalSort::get_ordered_list(&graph);

    for lib in mlirlib.iter().rev() {
        println!("cargo:rustc-link-lib=static={lib}");
    }
}
