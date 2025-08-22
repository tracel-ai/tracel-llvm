use std::{
    env,
    error::Error,
    ffi::OsStr,
    fs::read_dir,
    path::Path,
    process::{exit, Command},
    str,
};

const LLVM_MAJOR_VERSION: usize = 20;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    llvm_bundler_rs::bundler::bundle_cache()?;

    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=cc");
    println!("cargo:rustc-link-search={}", llvm_config("--libdir")?);

    build_c_library()?;

    for name in llvm_config("--libnames")?.trim().split(' ') {
        println!("cargo:rustc-link-lib=static={}", parse_library_name(name)?);
    }

    for flag in llvm_config("--system-libs")?.trim().split(' ') {
        let flag = flag.trim_start_matches("-l");

        if flag.starts_with('/') {
            // llvm-config returns absolute paths for dynamically linked libraries.
            let path = Path::new(flag);

            println!(
                "cargo:rustc-link-search={}",
                path.parent().unwrap().display()
            );
            println!(
                "cargo:rustc-link-lib={}",
                parse_library_name(path.file_name().unwrap().to_str().unwrap())?
            );
        } else {
            println!("cargo:rustc-link-lib={}", flag);
        }
    }

    if let Some(name) = get_system_libcpp() {
        println!("cargo:rustc-link-lib={name}");
    }

    bindgen::builder()
        .header("wrapper.h")
        .clang_arg("-Icc/include")
        .clang_arg("-I/usr/include")
        .clang_arg(format!("-I{}", llvm_config("--includedir")?))
        .default_enum_style(bindgen::EnumVariation::ModuleConsts)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()?
        .write_to_file(Path::new(&env::var("OUT_DIR")?).join("bindings.rs"))?;

    Ok(())
}

fn build_c_library() -> Result<(), Box<dyn Error>> {
    unsafe { env::set_var("CXXFLAGS", llvm_config("--cxxflags")?) };
    unsafe { env::set_var("CFLAGS", llvm_config("--cflags")?) };

    cc::Build::new()
        .cpp(true)
        .files(
            read_dir("cc/lib")?
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(|entry| entry.path())
                .filter(|path| path.is_file() && path.extension() == Some(OsStr::new("cpp"))),
        )
        .include("cc/include")
        .include("/usr/include")
        .include(llvm_config("--includedir")?)
        .flag("-Werror")
        .std("c++17")
        .opt_level(3)
        .compile("CTableGen");

    Ok(())
}

fn get_system_libcpp() -> Option<&'static str> {
    if cfg!(target_env = "msvc") {
        None
    } else if cfg!(target_os = "macos") {
        Some("c++")
    } else {
        Some("stdc++")
    }
}

fn llvm_config(argument: &str) -> Result<String, Box<dyn Error>> {
    let prefix = env::var(format!("TABLEGEN_{}0_PREFIX", LLVM_MAJOR_VERSION))
        .map(|path| Path::new(&path).join("bin"))?;

    let llvm_config_exe = if cfg!(target_os = "windows") {
        "llvm-config.exe"
    } else {
        "llvm-config"
    };

    let path = prefix.join(llvm_config_exe);

    let output = Command::new(path)
        .arg("--link-static")
        .arg(argument)
        .output()?;

    if !output.status.success() {
        let stderr = output.stderr;
        eprintln!("{}", str::from_utf8(&stderr)?.trim().to_owned());
        exit(1);
    }

    let stdout = output.stdout;
    Ok(str::from_utf8(&stdout)?.trim().to_string())
}

fn parse_library_name(name: &str) -> Result<&str, String> {
    name.strip_prefix("lib")
        .and_then(|name| name.split('.').next())
        .ok_or_else(|| format!("failed to parse library name: {name}"))
}
