use std::{
    env,
    error::Error,
    ffi::{OsStr, OsString},
    fs::read_dir,
    path::Path,
    process::exit,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let llvm_major_version = tracel_llvm_bundler::config::init()?;
    let prefix_env_var = format!("TABLEGEN_{llvm_major_version}0_PREFIX");
    println!("cargo:rerun-if-env-changed={prefix_env_var}");
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=cc");
    // Install prefix
    let prefix_os: Option<OsString> = env::var_os(prefix_env_var);
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

    build_c_library(prefix_os.as_ref())?;

    // clang built-in headers (resource dir)
    let clang_resource_dir = format!("{libdir}/clang/{llvm_major_version}");
    let linux_clang_includedir = format!("{clang_resource_dir}/include");

    let mut clang_args = vec!["-I", &includedir, "-I", "cc/include"];
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
        .default_enum_style(bindgen::EnumVariation::ModuleConsts)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()?
        .write_to_file(Path::new(&env::var("OUT_DIR")?).join("bindings.rs"))?;
    Ok(())
}

fn build_c_library(prefix_os: Option<&OsString>) -> Result<(), Box<dyn Error>> {
    let includedir = tracel_llvm_bundler::config::get_includedir(prefix_os)?;
    let mut b = cc::Build::new();
    let mut includes = vec!["cc/include"];
    let mut flags = vec![];
    if cfg!(target_os = "windows") {
        includes.push(includedir.as_str());
    } else {
        includes.extend(vec!["/usr/include"]);
        // -isystem suppresses warnings, if something is wrong in the resulted build, uncomment this line
        flags.push("-isystem");
        flags.push(includedir.as_str());
    }
    b.cpp(true)
        .files(
            read_dir("cc/lib")?
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(|e| e.path())
                .filter(|p| p.is_file() && p.extension() == Some(OsStr::new("cpp"))),
        )
        .warnings(false)
        .includes(includes)
        .flags(flags)
        .std("c++17");
    let cxxflags = tracel_llvm_bundler::config::get_cxxflags(prefix_os)?;
    apply_llvm_flags_to_cc(&mut b, &cxxflags);
    let cflags = tracel_llvm_bundler::config::get_cflags(prefix_os)?;
    apply_llvm_flags_to_cc(&mut b, &cflags);

    b.compile("CTableGen");
    Ok(())
}

fn apply_llvm_flags_to_cc(build: &mut cc::Build, flags: &str) {
    use std::collections::HashSet;
    let mut seen = HashSet::<String>::new();

    for raw in flags.split_whitespace() {
        let flag = raw.trim();
        if !seen.insert(flag.to_string()) {
            continue;
        }
        // Drop includes as we set them up ourselves
        if flag.starts_with("-I") || flag.starts_with("/I") {
            continue;
        }
        // Convert -DNAME[=VAL] into build.define to avoid special characters issues
        if let Some(def) = flag.strip_prefix("-D") {
            if let Some((k, v)) = def.split_once('=') {
                build.define(k, Some(v));
            } else {
                build.define(def, None);
            }
            continue;
        }
        build.flag_if_supported(flag);
    }
}
