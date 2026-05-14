use std::{
    env,
    error::Error,
    ffi::OsString,
    fs::{self, File},
    path::{Path, PathBuf},
    process::exit,
    time::Duration,
};

const CRATE_NAME: &str = "tracel-tblgen-rs";
const GITHUB_REPOSITORY: &str = "tracel-ai/tracel-llvm";

fn main() {
    // In xtask mode we skip the bundler installation altogether.
    if std::env::var_os("CARGO_FEATURE_XTASK").is_some() {
        println!("cargo:warning=xtask mode enabled, skipping bundle installation.");
        return;
    }

    if let Err(err) = run() {
        eprintln!("{err}");
        exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    prepare_bindings()?;

    let llvm_major = tracel_llvm_bundler::config::init()?;
    let prefix_var = format!("TABLEGEN_{llvm_major}0_PREFIX");

    println!("cargo:rerun-if-env-changed={prefix_var}");

    let prefix: Option<OsString> = env::var_os(&prefix_var);

    // Double check the version.
    let version = tracel_llvm_bundler::config::get_version(prefix.as_ref())?;
    if !version.starts_with(&format!("{llvm_major}.")) {
        return Err(
            format!("llvm-config version should be {llvm_major}.x.x (found {version})").into(),
        );
    }

    // Search paths.
    let libdir = tracel_llvm_bundler::config::get_libdir(prefix.as_ref())?;
    println!("cargo:rustc-link-search=native={libdir}");

    // Link configuration.
    for lib in tracel_llvm_bundler::config::get_libs(prefix.as_ref())? {
        println!("cargo:rustc-link-lib=static={lib}");
    }

    for sys in tracel_llvm_bundler::config::get_system_libs(prefix.as_ref())? {
        println!("cargo:rustc-link-lib={sys}");
    }

    if let Some(cpp) = tracel_llvm_bundler::config::get_system_libcpp() {
        println!("cargo:rustc-link-lib={cpp}");
    }

    // Link tblgen shim.
    println!("cargo:rustc-link-lib=static=CTableGen");

    tracel_llvm_bundler::config::set_homebrew_library_path()?;

    Ok(())
}

fn prepare_bindings() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=TRACEL_TBLGEN_RS_BINDINGS_URL");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_OS");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_ARCH");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR should be set by Cargo")?);

    let target_os = env::var("CARGO_CFG_TARGET_OS")?;
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH")?;

    let bindings_module_name = format!("bindings_{target_os}_{target_arch}.rs");
    let bindings_out_path = out_dir.join(&bindings_module_name);

    if bindings_out_path.exists() {
        return Ok(());
    }

    let url = match env::var("TRACEL_TBLGEN_RS_BINDINGS_URL") {
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
        .build()?;

    let mut resp = client.get(url).send()?.error_for_status()?;

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = File::create(dest)?;
    std::io::copy(&mut resp, &mut file)?;

    Ok(())
}
