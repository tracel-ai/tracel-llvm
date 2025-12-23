use std::{
    fs,
    path::{Path, PathBuf},
};

use tracel_xtask::prelude::{anyhow::Context as _, *};

use crate::utils::process::run_checked;

#[derive(Clone, Debug)]
pub(crate) struct CTableGenShimConfig {
    pub repo_root: PathBuf,
    pub bundle_install_dir: PathBuf,
    pub build_dir: PathBuf,
}

pub(crate) fn build_and_install_ctablegen_shim(cfg: CTableGenShimConfig) -> anyhow::Result<()> {
    let shim_src = cfg
        .repo_root
        .join("crates")
        .join("tracel-tblgen-rs")
        .join("cc");
    let shim_include = shim_src.join("include");
    let shim_lib_src = shim_src.join("lib");

    if !shim_lib_src.is_dir() {
        return Err(anyhow::anyhow!(
            "CTableGen shim sources should exist at {}",
            shim_lib_src.display()
        ));
    }

    if cfg.build_dir.exists() {
        fs::remove_dir_all(&cfg.build_dir)?;
    }
    fs::create_dir_all(&cfg.build_dir)?;

    // Use llvm-config from bundle to get flags and includes
    let llvm_config = cfg.bundle_install_dir.join("bin").join(if cfg!(windows) {
        "llvm-config.exe"
    } else {
        "llvm-config"
    });

    if !llvm_config.is_file() {
        return Err(anyhow::anyhow!(
            "llvm-config should exist at {}",
            llvm_config.display()
        ));
    }

    let llvm_cxxflags = run_capture(&llvm_config, &["--cxxflags"])?;
    let llvm_cppflags = run_capture_allow_fail(&llvm_config, &["--cppflags"])?;

    // Compile every .cpp into an object
    let mut objs: Vec<PathBuf> = Vec::new();
    let mut sources: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(&shim_lib_src)? {
        let p = entry?.path();
        if p.extension().and_then(|x| x.to_str()) == Some("cpp") {
            sources.push(p);
        }
    }
    sources.sort();

    if sources.is_empty() {
        return Err(anyhow::anyhow!(
            "No .cpp sources found in {}",
            shim_lib_src.display()
        ));
    }

    // Include bundle headers too
    let bundle_include = cfg.bundle_install_dir.join("include");

    if cfg!(windows) {
        // MSVC
        for src in sources {
            let obj = cfg.build_dir.join(format!(
                "{}.obj",
                src.file_stem().unwrap().to_string_lossy()
            ));
            let mut args: Vec<String> = vec![
                "/nologo".into(),
                "/c".into(),
                src.to_string_lossy().into_owned(),
                format!("/Fo{}", obj.to_string_lossy()),
                "/O2".into(),
                "/std:c++17".into(),
                "/EHsc".into(),
                "/MD".into(),
                format!("/I{}", shim_include.to_string_lossy()),
                format!("/I{}", bundle_include.to_string_lossy()),
            ];

            args.extend(split_flags_windows(&llvm_cppflags));
            args.extend(split_flags_windows(&llvm_cxxflags));

            run_checked("cl.exe", &args, None)?;
            objs.push(obj);
        }

        // Archive -> CTableGen.lib
        let out_lib = cfg.build_dir.join("CTableGen.lib");
        let mut lib_args: Vec<String> = vec![
            "/nologo".into(),
            format!("/out:{}", out_lib.to_string_lossy()),
        ];
        lib_args.extend(objs.iter().map(|p| p.to_string_lossy().into_owned()));
        run_checked("lib.exe", &lib_args, None)?;

        let install_lib = cfg.bundle_install_dir.join("lib");
        fs::create_dir_all(&install_lib)?;
        fs::copy(&out_lib, install_lib.join("CTableGen.lib"))?;
    } else {
        // clang++
        let cxx = std::env::var("CXX").unwrap_or_else(|_| "c++".into());

        for src in sources {
            let obj = cfg
                .build_dir
                .join(format!("{}.o", src.file_stem().unwrap().to_string_lossy()));

            let mut args: Vec<String> = vec![
                "-c".into(),
                src.to_string_lossy().into_owned(),
                "-o".into(),
                obj.to_string_lossy().into_owned(),
                "-O2".into(),
                "-fPIC".into(),
                "-std=c++17".into(),
                "-I".into(),
                shim_include.to_string_lossy().into_owned(),
                "-I".into(),
                bundle_include.to_string_lossy().into_owned(),
            ];
            args.extend(split_flags_unix(&llvm_cppflags));
            args.extend(split_flags_unix(&llvm_cxxflags));

            run_checked(&cxx, &args, None)?;
            objs.push(obj);
        }

        let out_a = cfg.build_dir.join("libCTableGen.a");
        let mut ar_args: Vec<String> = vec!["rcs".into(), out_a.to_string_lossy().into_owned()];
        ar_args.extend(objs.iter().map(|p| p.to_string_lossy().into_owned()));
        run_checked("ar", &ar_args, None)?;

        let install_lib = cfg.bundle_install_dir.join("lib");
        fs::create_dir_all(&install_lib)?;
        fs::copy(&out_a, install_lib.join("libCTableGen.a"))?;
    }

    Ok(())
}

fn run_capture(bin: &Path, args: &[&str]) -> anyhow::Result<String> {
    let out = std::process::Command::new(bin)
        .args(args)
        .output()
        .with_context(|| format!("{:?} should run", bin))?;
    if !out.status.success() {
        return Err(anyhow::anyhow!("{:?} {:?} should succeed", bin, args));
    }
    Ok(String::from_utf8(out.stdout)?.trim().to_string())
}

fn run_capture_allow_fail(bin: &Path, args: &[&str]) -> anyhow::Result<String> {
    let out = std::process::Command::new(bin).args(args).output()?;
    if !out.status.success() {
        return Ok(String::new());
    }
    Ok(String::from_utf8(out.stdout)?.trim().to_string())
}

fn split_flags_unix(s: &str) -> Vec<String> {
    s.split_whitespace().map(|x| x.to_string()).collect()
}

fn split_flags_windows(s: &str) -> Vec<String> {
    s.split_whitespace().map(|x| x.to_string()).collect()
}
