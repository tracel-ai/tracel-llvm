use std::{
    env,
    error::Error,
    ffi::OsString,
    fs::{self, File},
    path::{Path, PathBuf},
    process::exit,
    time::Duration,
};

const CRATE_NAME: &str = "tracel-mlir-sys";
const GITHUB_REPOSITORY: &str = "tracel-ai/tracel-llvm";

fn main() {
    // In xtask mode we skip the bundler installation altogether.
    if std::env::var_os("CARGO_FEATURE_XTASK").is_some() {
        println!("cargo:warning=xtask mode enabled, skipping bundle installation.");
        return;
    }

    if let Err(error) = run() {
        eprintln!("{error}");
        exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    prepare_bindings()?;

    let llvm_major_version = tracel_llvm_bundler::config::init()
        .map_err(|err| format!("tracel llvm bundler should initialize: {err}"))?;

    println!("cargo:rerun-if-changed=build.rs");

    let prefix_var = format!("MLIR_SYS_{llvm_major_version}0_PREFIX");
    println!("cargo:rerun-if-env-changed={prefix_var}");

    let prefix_os: Option<OsString> = env::var_os(&prefix_var);

    let libdir = tracel_llvm_bundler::config::get_libdir(prefix_os.as_ref())
        .map_err(|err| format!("LLVM library directory should be available: {err}"))?;
    println!("cargo:rustc-link-search=native={libdir}");

    for lib in tracel_llvm_bundler::config::get_libs(prefix_os.as_ref())
        .map_err(|err| format!("LLVM libraries should be available: {err}"))?
    {
        println!("cargo:rustc-link-lib=static={lib}");
    }

    for syslib in tracel_llvm_bundler::config::get_system_libs(prefix_os.as_ref())
        .map_err(|err| format!("LLVM system libraries should be available: {err}"))?
    {
        println!("cargo:rustc-link-lib={syslib}");
    }

    if let Some(name) = tracel_llvm_bundler::config::get_system_libcpp() {
        println!("cargo:rustc-link-lib={name}");
    }

    tracel_llvm_bundler::config::set_homebrew_library_path()
        .map_err(|err| format!("homebrew library path should be configured: {err}"))?;

    link_mlir_statically(llvm_major_version)?;

    Ok(())
}

fn prepare_bindings() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-env-changed=TRACEL_MLIR_SYS_BINDINGS_URL");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_OS");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_ARCH");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR should be set by Cargo")?);

    let target_os = env::var("CARGO_CFG_TARGET_OS")
        .map_err(|err| format!("CARGO_CFG_TARGET_OS should be set by Cargo: {err}"))?;
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH")
        .map_err(|err| format!("CARGO_CFG_TARGET_ARCH should be set by Cargo: {err}"))?;

    let bindings_module_name = format!("bindings_{target_os}_{target_arch}.rs");
    let bindings_out_path = out_dir.join(&bindings_module_name);

    if bindings_out_path.exists() {
        return Ok(());
    }

    let url = match env::var("TRACEL_MLIR_SYS_BINDINGS_URL") {
        Ok(url) => url,
        Err(_) => bindings_release_url(CRATE_NAME, &target_os, &target_arch),
    };

    download_to_path(&url, &bindings_out_path).map_err(|err| {
        format!("downloading generated bindings from {url} should succeed: {err}")
    })?;

    if !bindings_out_path.is_file() {
        return Err(format!(
            "Bindings download completed but expected file does not exist: {}",
            bindings_out_path.display()
        )
        .into());
    }

    Ok(())
}

fn bindings_release_url(crate_name: &str, target_os: &str, target_arch: &str) -> String {
    let version = tracel_llvm_bundler::config::TRACEL_LLVM_VERSION;
    let release_number = tracel_llvm_bundler::config::TRACEL_LLVM_RELEASE_NUMBER;

    let tag = format!("v{version}-{release_number}");
    let platform_stem = release_platform_stem(target_os, target_arch);
    let artifact_name = format!("{platform_stem}.{crate_name}.bindings.rs");

    format!("https://github.com/{GITHUB_REPOSITORY}/releases/download/{tag}/{artifact_name}")
}

fn release_platform_stem(target_os: &str, target_arch: &str) -> String {
    let arch = match target_arch {
        "x86_64" => "x64",
        "aarch64" => "AArch64",
        other => other,
    };

    format!("{target_os}-{arch}")
}

/// Download, overwriting, to a path.
fn download_to_path(url: &str, dest: &Path) -> Result<(), Box<dyn Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60 * 5))
        .build()
        .map_err(|err| format!("HTTP client should be created: {err}"))?;

    let mut resp = client
        .get(url)
        .send()
        .map_err(|err| format!("GET {url} should succeed: {err}"))?
        .error_for_status()
        .map_err(|err| format!("GET {url} should return a successful status: {err}"))?;

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "parent directory should be created: {}: {err}",
                parent.display()
            )
        })?;
    }

    let mut file = File::create(dest)
        .map_err(|err| format!("bindings file should be created: {}: {err}", dest.display()))?;

    std::io::copy(&mut resp, &mut file).map_err(|err| {
        format!(
            "response body should be written to {}: {err}",
            dest.display()
        )
    })?;

    Ok(())
}

fn link_mlir_statically(llvm_major_version: usize) -> Result<(), Box<dyn Error>> {
    use tracel_llvm_bundler::{
        dependency_graph::DependencyGraph, topological_sort::TopologicalSort,
    };

    let prefix_var = format!("MLIR_SYS_{llvm_major_version}0_PREFIX");
    let prefix =
        Path::new(&env::var(&prefix_var).map_err(|err| {
            format!("{prefix_var} prefix environment variable should exist: {err}")
        })?)
        .join("lib")
        .join("cmake")
        .join("mlir")
        .join("MLIRTargets.cmake");

    let graph = DependencyGraph::from_cmake(prefix)
        .map_err(|err| format!("MLIRTargets.cmake dependency graph should load: {err}"))?;
    let mlirlib = TopologicalSort::get_ordered_list(&graph);

    for lib in mlirlib.iter().rev() {
        println!("cargo:rustc-link-lib=static={lib}");
    }

    Ok(())
}
