use std::{env, ffi::OsString, path::Path};

const LLVM_FEATURE_PREFIX: &str = "CARGO_FEATURE_LLVM_";

fn main() {
    let llvm_major_version =
        tracel_llvm_bundler::config::init().expect("Should init tracel llvm bundler");

    println!("cargo:rerun-if-changed=build.rs");

    let llvm_version = tracel_llvm_bundler::config::llvm_version();

    // Either respect an explicit Cargo feature or set a default cfg(feature="llvm_...").
    select_or_set_llvm_feature(&llvm_version);

    // Then do all your link logic as before...
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

fn select_or_set_llvm_feature(detected_version: &str) {
    // detected_version: e.g. "20.1.4"
    // convert to "LLVM_20_1_4" for env var, "llvm_20_1_4" for feature name.
    let version_ident: String = detected_version
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();

    // Collect enabled llvm_* Cargo features (if any)
    let mut enabled_features = Vec::new();
    for (key, value) in env::vars() {
        if key.starts_with(LLVM_FEATURE_PREFIX) && value == "1" {
            enabled_features.push(key);
        }
    }

    match enabled_features.len() {
        0 => {
            // No version feature selected by Cargo. We set a default cfg(feature="llvm_...").
            let default_feature = format!("llvm_{version_ident}");
            println!("cargo:rustc-cfg=feature=\"{default_feature}\"");
        }
        1 => {
            // User explicitly selected a feature in Cargo.toml / CLI. Respect it.
            // Optional: verify it matches detected_version and panic if it doesn't.
            // let selected_env = &enabled_features[0]; // e.g. "CARGO_FEATURE_LLVM_20_1_4"
            // ...
        }
        _ => {
            // More than one llvm_* feature — this is invalid.
            panic!(
                "tracel-mlir-sys: Multiple llvm_* features enabled; \
                 select exactly one LLVM version feature."
            );
        }
    }
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
