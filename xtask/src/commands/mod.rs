pub(crate) mod bundle;
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
}

impl BundleWorkspace {
    pub fn new(workspace_dir: &Path) -> anyhow::Result<Self> {
        let platform = PlatformTriple::detect()?;
        let version = tracel_llvm_bundler::config::TRACEL_LLVM_VERSION.to_string();
        let release_number = tracel_llvm_bundler::config::TRACEL_LLVM_RELEASE_NUMBER.to_string();
        let workspace_dir = workspace_dir.to_path_buf();
        let llvm_project_dir = workspace_dir.join("llvm-project");
        let llvm_dir = llvm_project_dir.join("llvm");

        // runtime bundle layout
        let pkg_dir_name = format!("tracel-llvm-{}-{}", version, release_number);
        let bundle_install_dir = workspace_dir.join(&pkg_dir_name);
        let bundle_bin_dir = bundle_install_dir.join("bin");
        let bundle_build_dir = workspace_dir.join(".llvm_build");
        let bundle_lib_dir = bundle_install_dir.join("lib");

        Ok(Self {
            platform,
            version,
            release_number,
            workspace_dir,
            llvm_project_dir,
            llvm_dir,
            bundle_bin_dir,
            bundle_build_dir,
            bundle_install_dir,
            bundle_lib_dir,
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

    pub fn build_llvm_project(&self) -> anyhow::Result<()> {
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
                .with_context(|| "bundle build directory should be deleted")?;
        }
        if self.bundle_install_dir.exists() {
            fs::remove_dir_all(&self.bundle_install_dir)
                .with_context(|| "bundle install directory should be deleted")?;
        }

        let cfg = LlvmCmakeBuild {
            build_dir: self.bundle_build_dir.clone(),
            install_dir: self.bundle_install_dir.clone(),
            extra_cmake_args: vec![
                "-DLLVM_ENABLE_PROJECTS=lld".into(),
                "-DLLVM_BUILD_EXAMPLES=OFF".into(),
                "-DLLVM_BUILD_TESTS=OFF".into(),
                "-DLLVM_BUILD_TOOLS=OFF".into(),
                "-DLLVM_ENABLE_DIA_SDK=OFF".into(),
                "-DLLVM_ENABLE_DUMP=ON".into(),
                "-DLLVM_ENABLE_LIBEDIT=OFF".into(),
                "-DLLVM_ENABLE_LIBXML2=OFF".into(),
                "-DLLVM_ENABLE_LTO=OFF".into(),
                "-DLLVM_ENABLE_RTTI=ON".into(),
                "-DLLVM_ENABLE_SPHINX=OFF".into(),
                "-DLLVM_ENABLE_ZLIB=OFF".into(),
                "-DLLVM_INCLUDE_DOCS=OFF".into(),
                "-DLLVM_INCLUDE_EXAMPLES=OFF".into(),
                "-DLLVM_INCLUDE_TESTS=OFF".into(),
                "-DLLVM_INCLUDE_TOOLS=ON".into(),
                "-DLLVM_ENABLE_WARNINGS=OFF".into(),
            ],
            ninja_targets_before_install: vec!["llvm-config".into()],
        };

        group_info!("BundleWorkspace: cmake configure (LLVM bundle)");
        self.cmake_configure(&cfg)?;
        endgroup!();

        group_info!("BundleWorkspace: ninja build+install (LLVM bundle)");
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
            "-DBUILD_SHARED_LIBS=OFF".into(),
            format!(
                "-DCMAKE_INSTALL_PREFIX={}",
                cfg.install_dir.to_string_lossy()
            ),
            #[cfg(not(target_os = "macos"))]
            "-DLLVM_TARGETS_TO_BUILD=host;AMDGPU".into(),
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
    extra_cmake_args: Vec<String>,
    ninja_targets_before_install: Vec<String>,
}
