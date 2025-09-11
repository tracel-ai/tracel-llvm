use std::{
    env,
    error::Error,
    ffi::{OsStr, OsString},
    fs::read_dir,
    path::Path,
    process::exit,
};

const LLVM_MAJOR_VERSION: usize = 20;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=cc");
    // Build cache
    llvm_bundler_rs::bundler::bundle_cache()?;
    // Install prefix
    let prefix_os: Option<OsString> = env::var_os(format!("TABLEGEN_{LLVM_MAJOR_VERSION}0_PREFIX"));
    // Version gate
    let version = llvm_bundler_rs::config::get_version(prefix_os.as_ref())?;
    if !version.starts_with(&format!("{LLVM_MAJOR_VERSION}.")) {
        return Err(format!(
            "failed to find correct version ({LLVM_MAJOR_VERSION}.x.x) of llvm-config (found {version})"
        )
                   .into());
    }
    // Libraries and headers
    let includedir = llvm_bundler_rs::config::get_includedir(prefix_os.as_ref())?;
    let libdir = llvm_bundler_rs::config::get_libdir(prefix_os.as_ref())?;
    println!("cargo:rustc-link-search=native={libdir}");
    for lib in llvm_bundler_rs::config::get_libs(prefix_os.as_ref())? {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    for syslib in llvm_bundler_rs::config::get_system_libs(prefix_os.as_ref())? {
        println!("cargo:rustc-link-lib={syslib}");
    }
    if let Some(name) = llvm_bundler_rs::config::get_system_libcpp() {
        println!("cargo:rustc-link-lib={name}");
    }
    build_c_library(prefix_os.as_ref())?;

    bindgen::builder()
        .header("wrapper.h")
        .clang_args(["-I", &includedir])
        .clang_args(["-I", "/usr/include"])
        .clang_args(["-I", "cc/include"])
        .default_enum_style(bindgen::EnumVariation::ModuleConsts)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()?
        .write_to_file(Path::new(&env::var("OUT_DIR")?).join("bindings.rs"))?;
    Ok(())
}

fn build_c_library(prefix_os: Option<&OsString>) -> Result<(), Box<dyn Error>> {
    let cxxflags = llvm_bundler_rs::config::get_cxxflags(prefix_os)?;
    let cflags   = llvm_bundler_rs::config::get_cflags(prefix_os)?;
    let includedir = llvm_bundler_rs::config::get_includedir(prefix_os)?;

    let mut b = cc::Build::new();
    b.cpp(true)
        .files(
            read_dir("cc/lib")?
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(|e| e.path())
                .filter(|p| p.is_file() && p.extension() == Some(OsStr::new("cpp")))
        )
        .include("cc/include")
        .include("/usr/include")
        // suppress warnings, if something is wrong in the resulted build, uncomment this line
        .flag("-isystem")
        .flag(&includedir)
        .std("c++17")
        .opt_level(3);
    apply_llvm_flags_to_cc(&mut b, &cxxflags);
    apply_llvm_flags_to_cc(&mut b, &cflags);

    b.compile("CTableGen");
    Ok(())
}

fn apply_llvm_flags_to_cc(build: &mut cc::Build, flags: &str) {
    for tok in flags.split_whitespace() {
        if tok.starts_with("-I") {
            // Drop all -I... from llvm-config. We’ll add includes with .include().
            // The reason we do this is that paths may contain spaces and this will break
            // if we pass them directly from the llvm-config output
            continue;
        }
        if let Some(def) = tok.strip_prefix("-D") {
            // -DNAME[=VALUE]
            if let Some((name, val)) = def.split_once('=') {
                build.define(name, Some(val));
            } else {
                build.define(def, None);
            }
            continue;
        }
        // Pass through other flags (e.g., -fno-exceptions, -fno-rtti, etc.)
        build.flag_if_supported(tok);
    }
}
