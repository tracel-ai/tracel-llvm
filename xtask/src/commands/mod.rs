pub(crate) mod bindgen;
pub(crate) mod bundle;
pub(crate) mod generators;
pub(crate) mod setup;

use std::{
    fs,
    path::{Path, PathBuf},
};

use tracel_xtask::prelude::{anyhow::Context as _, *};

use crate::utils::{
    git::git_clone_shallow_tag,
    platform::PlatformTriple,
    process::{require_tools, run_checked},
};

/// Centralized configuration for the `.llvm` workspace.
#[derive(Debug, Clone)]
pub(crate) struct BundleWorkspace {
    // Versioning
    pub platform: PlatformTriple,
    pub version: String,
    pub release_number: String,

    // Root
    pub workspace_dir: PathBuf,

    // Source checkout
    pub llvm_project_dir: PathBuf,
    pub llvm_dir: PathBuf,

    // Bundle build
    pub bundle_build_dir: PathBuf,
    pub bundle_install_dir: PathBuf,
    pub bundle_bin_dir: PathBuf,
    pub bundle_lib_dir: PathBuf,
    pub bundle_include_dir: PathBuf,

    // Clang toolchain build used only for Rust bindgen
    pub clang_build_dir: PathBuf,
    pub clang_include_dir: PathBuf,
    pub clang_install_dir: PathBuf,
    pub clang_lib_dir: PathBuf,
    pub clang_resource_dir: PathBuf,
    pub clang_resource_include_dir: PathBuf,
}

impl BundleWorkspace {
    pub fn new(workspace_dir: &Path) -> anyhow::Result<Self> {
        let platform = PlatformTriple::detect()?;
        let version = tracel_llvm_bundler::config::TRACEL_LLVM_VERSION.to_string();
        let major = llvm_major_version_from(&version)?;
        let release_number = tracel_llvm_bundler::config::TRACEL_LLVM_RELEASE_NUMBER.to_string();
        let workspace_dir = workspace_dir.to_path_buf();
        let llvm_project_dir = workspace_dir.join("llvm-project");
        let llvm_dir = llvm_project_dir.join("llvm");

        // platform-specific libdir name
        let mlir_libdir_name = libdir_name_for_platform(&platform);
        let clang_libdir_name = libdir_name_for_platform(&platform);

        // mlir runtime bundle layout
        let pkg_dir_name = format!("tracel-llvm-{}-{}", version, release_number);
        let mlir_install_dir = workspace_dir.join(&pkg_dir_name);
        let mlir_bin_dir = mlir_install_dir.join("bin");
        let mlir_build_dir = workspace_dir.join(".mlir_build");
        let mlir_include_dir = mlir_install_dir.join("include");
        let mlir_lib_dir = mlir_install_dir.join(mlir_libdir_name);

        // clang bindgen toolchain layout
        let clang_install_dir = workspace_dir.join(".clang-bindgen");
        let clang_build_dir = workspace_dir.join(".clang-bindgen-build");
        let clang_include_dir = clang_install_dir.join("include");
        let clang_lib_dir = clang_install_dir.join(clang_libdir_name);
        let clang_resource_dir = clang_lib_dir.join("clang").join(major.to_string());
        let clang_resource_include_dir = clang_resource_dir.join("include");

        Ok(Self {
            platform,
            version,
            release_number,
            workspace_dir,
            llvm_project_dir,
            llvm_dir,
            bundle_bin_dir: mlir_bin_dir,
            bundle_build_dir: mlir_build_dir,
            bundle_include_dir: mlir_include_dir,
            bundle_install_dir: mlir_install_dir,
            bundle_lib_dir: mlir_lib_dir,
            clang_build_dir,
            clang_include_dir,
            clang_install_dir,
            clang_lib_dir,
            clang_resource_dir,
            clang_resource_include_dir,
        })
    }

    /// Ensures the workspace root exists.
    pub fn ensure_workspace_dir(&self) -> anyhow::Result<()> {
        fs::create_dir_all(&self.workspace_dir)
            .with_context(|| "workspace directory should be created")?;
        Ok(())
    }

    /// Clones llvm-project into the workspace at llvmorg-<version>.
    pub fn clone_llvm_project_fresh(&self) -> anyhow::Result<()> {
        require_tools(&["git"])?;
        self.ensure_workspace_dir()?;

        if self.llvm_project_dir.exists() {
            fs::remove_dir_all(&self.llvm_project_dir)
                .with_context(|| "llvm-project directory should be deleted")?;
        }

        let tag = format!("llvmorg-{}", self.version);

        group_info!("BundleWorkspace: clone llvm-project @ {}", tag);
        git_clone_shallow_tag(
            "https://github.com/llvm/llvm-project.git",
            &tag,
            &self.llvm_project_dir,
        )?;
        endgroup!();

        Ok(())
    }

    pub fn build_mlir_project(&self) -> anyhow::Result<()> {
        require_tools(&["cmake", "ninja"])?;
        self.ensure_workspace_dir()?;

        if !self.llvm_dir.exists() {
            return Err(anyhow::anyhow!(
                "LLVM sources not found at '{}'. Run `cx bundle build` (or clone) first.",
                self.llvm_dir.display()
            ));
        }

        if self.bundle_build_dir.exists() {
            fs::remove_dir_all(&self.bundle_build_dir)
                .with_context(|| "mlir build directory should be deleted")?;
        }
        if self.bundle_install_dir.exists() {
            fs::remove_dir_all(&self.bundle_install_dir)
                .with_context(|| "mlir install directory should be deleted")?;
        }

        let cfg = LlvmCmakeBuild {
            build_dir: self.bundle_build_dir.clone(),
            install_dir: self.bundle_install_dir.clone(),
            projects: "mlir".to_string(),
            shared_libs: false,
            extra_cmake_args: vec![
                "-DLLVM_INCLUDE_TOOLS=ON".into(),
                "-DLLVM_BUILD_TOOLS=OFF".into(),
                "-DLLVM_BUILD_TESTS=OFF".into(),
                "-DLLVM_INCLUDE_TESTS=OFF".into(),
                "-DLLVM_BUILD_EXAMPLES=OFF".into(),
                "-DLLVM_INCLUDE_EXAMPLES=OFF".into(),
                "-DLLVM_INCLUDE_DOCS=OFF".into(),
                "-DLLVM_ENABLE_ZLIB=OFF".into(),
                "-DLLVM_ENABLE_LIBXML2=OFF".into(),
                "-DLLVM_ENABLE_LIBEDIT=OFF".into(),
                "-DLLVM_ENABLE_LTO=OFF".into(),
                "-DLLVM_ENABLE_SPHINX=OFF".into(),
                "-DLLVM_ENABLE_RTTI=ON".into(),
            ],
            ninja_targets_before_install: vec!["llvm-config".into()],
        };

        group_info!("BundleWorkspace: cmake configure (MLIR bundle)");
        self.cmake_configure(&cfg)?;
        endgroup!();

        group_info!("BundleWorkspace: ninja build+install (MLIR bundle)");
        self.ninja_build_and_install(&cfg)?;
        endgroup!();

        Ok(())
    }

    pub fn build_clang_for_bindgen(&self, rebuild: bool) -> anyhow::Result<()> {
        require_tools(&["cmake", "ninja"])?;
        self.ensure_workspace_dir()?;

        if !self.llvm_dir.exists() {
            return Err(anyhow::anyhow!(
                "LLVM sources not found at '{}'. Run `cx bundle build` (or clone) first.",
                self.llvm_dir.display()
            ));
        }

        if rebuild {
            if self.clang_build_dir.exists() {
                fs::remove_dir_all(&self.clang_build_dir)
                    .with_context(|| "clang bindgen build directory should be deleted")?;
            }
            if self.clang_install_dir.exists() {
                fs::remove_dir_all(&self.clang_install_dir)
                    .with_context(|| "clang bindgen install directory should be deleted")?;
            }
        }

        let cfg = LlvmCmakeBuild {
            build_dir: self.clang_build_dir.clone(),
            install_dir: self.clang_install_dir.clone(),
            projects: "clang".to_string(),
            shared_libs: true,
            extra_cmake_args: vec![
                "-DLLVM_BUILD_TESTS=OFF".into(),
                "-DLLVM_INCLUDE_TESTS=OFF".into(),
                "-DLLVM_BUILD_EXAMPLES=OFF".into(),
                "-DLLVM_INCLUDE_EXAMPLES=OFF".into(),
                "-DLLVM_INCLUDE_DOCS=OFF".into(),
                "-DLLVM_ENABLE_ZLIB=OFF".into(),
                "-DLLVM_ENABLE_LIBXML2=OFF".into(),
                "-DLLVM_ENABLE_LIBEDIT=OFF".into(),
                "-DLLVM_ENABLE_LTO=OFF".into(),
                "-DLLVM_ENABLE_SPHINX=OFF".into(),
                "-DLLVM_ENABLE_RTTI=ON".into(),
            ],
            ninja_targets_before_install: vec!["libclang".into()],
        };

        group_info!("BundleWorkspace: cmake configure (Clang bindgen toolchain)");
        self.cmake_configure(&cfg)?;
        endgroup!();

        group_info!("BundleWorkspace: ninja build+install (Clang bindgen toolchain)");
        self.ninja_build_and_install(&cfg)?;
        endgroup!();

        Ok(())
    }

    fn cmake_configure(&self, cfg: &LlvmCmakeBuild) -> anyhow::Result<()> {
        fs::create_dir_all(&cfg.build_dir).with_context(|| "build directory should be created")?;

        let mut args = vec![
            "-S".into(),
            self.llvm_dir.to_string_lossy().into_owned(),
            "-B".into(),
            cfg.build_dir.to_string_lossy().into_owned(),
            "-G".into(),
            "Ninja".into(),
            "-DCMAKE_BUILD_TYPE=Release".into(),
            format!("-DCMAKE_INSTALL_PREFIX={}", cfg.install_dir.to_string_lossy()),
            format!(
                "-DBUILD_SHARED_LIBS={}",
                if cfg.shared_libs { "ON" } else { "OFF" }
            ),
            format!("-DLLVM_ENABLE_PROJECTS={}", cfg.projects),
            "-DLLVM_TARGETS_TO_BUILD=host".into(),
        ];

        args.extend(cfg.extra_cmake_args.clone());

        run_checked("cmake", &args, None)?;
        Ok(())
    }

    fn ninja_build_and_install(&self, cfg: &LlvmCmakeBuild) -> anyhow::Result<()> {
        let mut args = vec!["-C".into(), cfg.build_dir.to_string_lossy().into_owned()];
        args.extend(cfg.ninja_targets_before_install.clone());
        args.push("install".into());

        run_checked("ninja", &args, None)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct LlvmCmakeBuild {
    build_dir: PathBuf,
    install_dir: PathBuf,
    projects: String,
    shared_libs: bool,
    extra_cmake_args: Vec<String>,
    ninja_targets_before_install: Vec<String>,
}

fn libdir_name_for_platform(platform: &PlatformTriple) -> &'static str {
    let stem = platform.archive_stem();

    // examples: "linux-x64", "linux-AArch64", "macos-AArch64", "windows-x64"
    if stem.starts_with("linux-") {
        if stem.contains("x64") {
            "lib64"
        } else {
            "lib"
        }
    } else {
        // windows + macos
        "lib"
    }
}

fn llvm_major_version_from(version: &str) -> anyhow::Result<usize> {
    let major = version
        .split('.')
        .next()
        .ok_or_else(|| anyhow::anyhow!("LLVM version should have a major component"))?
        .parse::<usize>()
        .with_context(|| "LLVM major version should parse")?;
    Ok(major)
}
