use dirs::data_local_dir;
use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{build_support::install::install_bundle, llvm_config_manifest::LlvmConfigManifest};

const TRACEL_LLVM_CACHE_DIRECTORY_NAME: &str = "tracel";
const TRACEL_LLVM_CACHE_PREFIX: &str = "tracel-llvm";
const TRACEL_LLVM_ARTIFACT_BASE_URL: &str =
    "https://github.com/tracel-ai/tracel-llvm/releases/download";
pub const TRACEL_LLVM_VERSION: &str = "23.1.0";
pub const TRACEL_LLVM_RELEASE_NUMBER: &str = "4";
pub const TRACEL_LLVM_FULL_VERSION: &str =
    constcat::concat!(TRACEL_LLVM_VERSION, "-", TRACEL_LLVM_RELEASE_NUMBER);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Architecture {
    X86_64,
    AArch64,
}

impl Architecture {
    pub fn current() -> Architecture {
        if cfg!(target_arch = "x86_64") {
            Architecture::X86_64
        } else if cfg!(target_arch = "aarch64") {
            Architecture::AArch64
        } else {
            panic!("Unsupported target architecture for tracel-llvm-bundler.")
        }
    }

    /// The architecture of a `target_arch` cfg value, such as `CARGO_CFG_TARGET_ARCH`.
    pub fn of_target_arch(target_arch: &str) -> Option<Self> {
        match target_arch {
            "x86_64" => Some(Self::X86_64),
            "aarch64" => Some(Self::AArch64),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperatingSystem {
    Linux,
    MacOs,
    Windows,
}

impl OperatingSystem {
    pub fn current() -> Self {
        if cfg!(target_os = "linux") {
            Self::Linux
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            panic!("Unsupported operating system for tracel-llvm-bundler.")
        }
    }

    /// The operating system of a `target_os` cfg value, such as `CARGO_CFG_TARGET_OS`.
    pub fn of_target_os(target_os: &str) -> Option<Self> {
        match target_os {
            "linux" => Some(Self::Linux),
            "macos" => Some(Self::MacOs),
            "windows" => Some(Self::Windows),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Environment {
    pub os: OperatingSystem,
    pub arch: Architecture,
}

impl Environment {
    /// The machine running this code.
    pub fn current() -> Self {
        Self {
            os: OperatingSystem::current(),
            arch: Architecture::current(),
        }
    }

    /// The platform being built for.
    ///
    /// In a build script, that is the script's target, read from
    /// `CARGO_CFG_TARGET_OS` and `CARGO_CFG_TARGET_ARCH`: when cross-compiling it
    /// is not the machine running the script. Outside a build script, it is
    /// [`Environment::current`].
    pub fn of_the_build_target() -> ConfigResult<Self> {
        match (
            env::var("CARGO_CFG_TARGET_OS"),
            env::var("CARGO_CFG_TARGET_ARCH"),
        ) {
            (Ok(target_os), Ok(target_arch)) => Self::of_target(&target_os, &target_arch),
            _ => Ok(Self::current()),
        }
    }

    /// The platform of a target's `target_os` and `target_arch` cfg values.
    pub fn of_target(target_os: &str, target_arch: &str) -> ConfigResult<Self> {
        Ok(Self {
            os: OperatingSystem::of_target_os(target_os).ok_or(ConfigError::UnsupportedSystem)?,
            arch: Architecture::of_target_arch(target_arch)
                .ok_or(ConfigError::UnsupportedSystem)?,
        })
    }

    /// Where this platform's bundle is installed: each platform has its own
    /// directory, so a machine can hold its own bundle and those it cross-compiles for.
    pub fn llvm_path(&self) -> ConfigResult<PathBuf> {
        data_local_dir()
            .map(|data_local| self.llvm_path_under(&data_local))
            .ok_or(ConfigError::UnsupportedSystem)
    }

    fn llvm_path_under(&self, data_local: &Path) -> PathBuf {
        data_local
            .join(TRACEL_LLVM_CACHE_DIRECTORY_NAME)
            .join(self.platform())
            .join(format!(
                "{TRACEL_LLVM_CACHE_PREFIX}-{TRACEL_LLVM_FULL_VERSION}"
            ))
    }

    pub fn artifact_cache_path(&self) -> anyhow::Result<PathBuf> {
        let base = Self::artifact_cache_dir()?;
        Ok(base.join(self.cache_filename()))
    }

    pub fn checksum_cache_path(&self) -> anyhow::Result<PathBuf> {
        let base = Self::artifact_cache_dir()?;
        Ok(base.join(self.checksum_cache_filename()))
    }

    pub fn sentinel_path(&self, llvm_path: &Path) -> PathBuf {
        llvm_path.join(self.sentinel_name())
    }

    fn platform(&self) -> &'static str {
        match (&self.os, &self.arch) {
            (OperatingSystem::Linux, Architecture::X86_64) => "linux-x64",
            (OperatingSystem::Linux, Architecture::AArch64) => "linux-AArch64",
            (OperatingSystem::MacOs, Architecture::X86_64) => "macos-x64",
            (OperatingSystem::MacOs, Architecture::AArch64) => "macos-AArch64",
            (OperatingSystem::Windows, Architecture::X86_64) => "windows-x64",
            (OperatingSystem::Windows, Architecture::AArch64) => "windows-AArch64",
        }
    }

    fn filename(&self) -> String {
        format!("{}.tar.xz", self.platform())
    }

    fn checksum_filename(&self) -> String {
        self.filename().replace(".tar.xz", ".checksums.json")
    }

    pub fn artifact_url(&self) -> String {
        let filename = self.filename();
        format!("{TRACEL_LLVM_ARTIFACT_BASE_URL}/v{TRACEL_LLVM_FULL_VERSION}/{filename}")
    }

    pub fn checksum_url(&self) -> String {
        let filename = self.checksum_filename();
        format!("{TRACEL_LLVM_ARTIFACT_BASE_URL}/v{TRACEL_LLVM_FULL_VERSION}/{filename}")
    }

    pub fn artifact_cache_dir() -> anyhow::Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        let base = home.join(".cache").join("tracel");
        fs::create_dir_all(&base)?;
        Ok(base)
    }

    fn cache_filename(&self) -> String {
        format!("tracel-llvm-{TRACEL_LLVM_FULL_VERSION}-{}", self.filename())
    }

    fn checksum_cache_filename(&self) -> String {
        format!(
            "tracel-llvm-{TRACEL_LLVM_FULL_VERSION}-{}",
            self.checksum_filename()
        )
    }

    fn sentinel_name(&self) -> &'static str {
        ".tracel-llvm-installed"
    }
}

pub type ConfigResult<T> = std::result::Result<T, ConfigError>;

#[derive(Debug)]
pub enum ConfigError {
    UnsupportedSystem,
    IoError(std::io::Error),
    Utf8(std::str::Utf8Error),
    ToolExit {
        path: String,
        status: i32,
        stderr: String,
    },
    Install(anyhow::Error),
    Manifest(serde_json::Error),
}

impl std::error::Error for ConfigError {}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::UnsupportedSystem => write!(f, "Unsupported system"),
            ConfigError::IoError(error) => write!(f, "{error}"),
            ConfigError::Utf8(error) => write!(f, "{error}"),
            ConfigError::ToolExit {
                path,
                status,
                stderr,
            } => {
                if stderr.is_empty() {
                    write!(f, "`{path}` exited with status {status}")
                } else {
                    write!(f, "`{path}` exited with status {status}: {stderr}")
                }
            }
            ConfigError::Install(error) => write!(f, "installing the LLVM bundle: {error:#}"),
            ConfigError::Manifest(error) => {
                write!(f, "reading the bundle's llvm-config manifest: {error}")
            }
        }
    }
}

impl From<std::io::Error> for ConfigError {
    fn from(value: std::io::Error) -> Self {
        ConfigError::IoError(value)
    }
}
impl From<std::str::Utf8Error> for ConfigError {
    fn from(value: std::str::Utf8Error) -> Self {
        ConfigError::Utf8(value)
    }
}

pub fn llvm_version() -> String {
    TRACEL_LLVM_VERSION.replace('.', "_")
}

/// The install prefix of the bundle for the platform being built, installed on
/// first use.
///
/// Meant to be called from a `build.rs`: the bundle is the one of that build
/// script's target (see [`Environment::of_the_build_target`]), so cross-compiling
/// links the target's LLVM rather than the build machine's.
pub fn llvm_path() -> ConfigResult<PathBuf> {
    let environment = Environment::of_the_build_target()?;
    let path = environment.llvm_path()?;
    install_bundle(&environment, &path).map_err(ConfigError::Install)?;
    Ok(path)
}

/// Records what `llvm-config` answers for the bundle installed at `prefix` into
/// the bundle itself, for the machines that cannot run it: see
/// [`LlvmConfigManifest`].
pub fn record_llvm_config_manifest(prefix: &Path) -> ConfigResult<LlvmConfigManifest> {
    let prefix_os = prefix.as_os_str().to_os_string();
    let prefix_os = Some(&prefix_os);
    let includedir = llvm_config(prefix_os, "--includedir")?;
    let bare_names = |answer: String| -> Vec<String> {
        answer.split_whitespace().filter_map(norm_lib).collect()
    };

    let manifest = LlvmConfigManifest {
        version: llvm_config(prefix_os, "--version")?,
        libs: bare_names(llvm_config(prefix_os, "--libs")?),
        libnames: bare_names(llvm_config(prefix_os, "--libnames")?),
        system_libs: bare_names(llvm_config(prefix_os, "--system-libs")?),
        cxxflags: flags_after_include(&llvm_config(prefix_os, "--cxxflags")?, &includedir),
        cflags: flags_after_include(&llvm_config(prefix_os, "--cflags")?, &includedir),
    };
    manifest.write(prefix)?;
    Ok(manifest)
}

/// The manifest of the bundle at `prefix_os`, with that prefix, when it has one.
fn recorded(prefix_os: Option<&OsString>) -> ConfigResult<Option<(PathBuf, LlvmConfigManifest)>> {
    let Some(prefix) = prefix_os.map(PathBuf::from) else {
        return Ok(None);
    };
    Ok(LlvmConfigManifest::read(&prefix)?.map(|manifest| (prefix, manifest)))
}

fn path_string(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

fn norm_lib(token: &str) -> Option<String> {
    let mut s = token.trim().trim_matches('"');
    if s.is_empty() {
        return None;
    }
    // Unix style
    if let Some(rest) = s.strip_prefix("-l") {
        s = rest;
    }
    // Windows path or *.lib
    if s.contains('\\') || s.contains('/') || s.ends_with(".lib") {
        return Path::new(s)
            .file_stem()
            .map(|x| x.to_string_lossy().into_owned());
    }
    Some(s.to_owned())
}

/// Returns a vector of all libraries required by LLVM (bare names).
pub fn get_libs(prefix_os: Option<&OsString>) -> ConfigResult<Vec<String>> {
    if let Some((_, manifest)) = recorded(prefix_os)? {
        return Ok(manifest.libs);
    }
    let libs = llvm_config(prefix_os, "--libs")?;
    Ok(libs.split_whitespace().filter_map(norm_lib).collect())
}

/// Returns a vector of all library names required by LLVM (bare names).
pub fn get_libnames(prefix_os: Option<&OsString>) -> ConfigResult<Vec<String>> {
    if let Some((_, manifest)) = recorded(prefix_os)? {
        return Ok(manifest.libnames);
    }
    let libs = llvm_config(prefix_os, "--libnames")?;
    Ok(libs.split_whitespace().filter_map(norm_lib).collect())
}

/// Returns a vector of all system libraries required by LLVM (bare names).
pub fn get_system_libs(prefix_os: Option<&OsString>) -> ConfigResult<Vec<String>> {
    if let Some((_, manifest)) = recorded(prefix_os)? {
        return Ok(manifest.system_libs);
    }
    let libs = llvm_config(prefix_os, "--system-libs")?;
    Ok(libs.split_whitespace().filter_map(norm_lib).collect())
}

/// Returns the lib directory path
pub fn get_libdir(prefix_os: Option<&OsString>) -> ConfigResult<String> {
    if let Some((prefix, _)) = recorded(prefix_os)? {
        return Ok(path_string(prefix.join("lib")));
    }
    let libdir = llvm_config(prefix_os, "--libdir")?;
    Ok(libdir)
}

/// Returns the includes directory path.
pub fn get_includedir(prefix_os: Option<&OsString>) -> ConfigResult<String> {
    if let Some((prefix, _)) = recorded(prefix_os)? {
        return Ok(path_string(prefix.join("include")));
    }
    let includedir = llvm_config(prefix_os, "--includedir")?;
    Ok(includedir)
}

/// Returns the LLVM version.
pub fn get_version(prefix_os: Option<&OsString>) -> ConfigResult<String> {
    if let Some((_, manifest)) = recorded(prefix_os)? {
        return Ok(manifest.version);
    }
    let version = llvm_config(prefix_os, "--version")?;
    Ok(version)
}

/// Returns the CXX flags
pub fn get_cxxflags(prefix_os: Option<&OsString>) -> ConfigResult<String> {
    if let Some((_, manifest)) = recorded(prefix_os)? {
        return Ok(joined_with_include(
            &get_includedir(prefix_os)?,
            &manifest.cxxflags,
        ));
    }
    let flags = llvm_config(prefix_os, "--cxxflags")?;
    Ok(flags)
}

/// Returns the C flags with some tweaks for portability
pub fn get_cflags(prefix_os: Option<&OsString>) -> ConfigResult<String> {
    if let Some((_, manifest)) = recorded(prefix_os)? {
        return Ok(joined_with_include(
            &get_includedir(prefix_os)?,
            &manifest.cflags,
        ));
    }
    let flags = llvm_config(prefix_os, "--cflags")?;
    Ok(flags)
}

/// Returns the CXX flags as individual compiler arguments.
///
/// Prefer this over [`get_cxxflags`], whose raw string cannot be split on
/// whitespace: see [`split_flags`].
pub fn get_cxxflags_args(prefix_os: Option<&OsString>) -> ConfigResult<Vec<OsString>> {
    if let Some((_, manifest)) = recorded(prefix_os)? {
        return Ok(with_include(&get_includedir(prefix_os)?, manifest.cxxflags));
    }
    let flags = get_cxxflags(prefix_os)?;
    Ok(split_flags(&flags, &get_includedir(prefix_os)?))
}

/// Returns the C flags as individual compiler arguments.
///
/// Prefer this over [`get_cflags`], whose raw string cannot be split on
/// whitespace: see [`split_flags`].
pub fn get_cflags_args(prefix_os: Option<&OsString>) -> ConfigResult<Vec<OsString>> {
    if let Some((_, manifest)) = recorded(prefix_os)? {
        return Ok(with_include(&get_includedir(prefix_os)?, manifest.cflags));
    }
    let flags = get_cflags(prefix_os)?;
    Ok(split_flags(&flags, &get_includedir(prefix_os)?))
}

/// Splits a raw `llvm-config` flag string into individual compiler arguments.
fn split_flags(flags: &str, includedir: &str) -> Vec<OsString> {
    with_include(includedir, flags_after_include(flags, includedir))
}

/// The arguments of a raw `llvm-config` flag string other than the include flag.
///
/// The include directory can hold spaces (`Library/Application Support` on
/// macOS), so it is taken out before splitting on whitespace.
fn flags_after_include(flags: &str, includedir: &str) -> Vec<String> {
    flags
        .replace(&format!("-I{includedir}"), " ")
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

fn with_include(includedir: &str, flags: Vec<String>) -> Vec<OsString> {
    std::iter::once(OsString::from(format!("-I{includedir}")))
        .chain(flags.into_iter().map(OsString::from))
        .collect()
}

fn joined_with_include(includedir: &str, flags: &[String]) -> String {
    std::iter::once(format!("-I{includedir}"))
        .chain(flags.iter().cloned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The C++ runtime LLVM was built against, for the target of the calling build
/// script (see [`Environment::of_the_build_target`]).
pub fn get_system_libcpp() -> Option<&'static str> {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_else(|_| env::consts::OS.into());
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_else(|_| {
        if cfg!(target_env = "msvc") {
            "msvc"
        } else {
            ""
        }
        .into()
    });
    system_libcpp_of(&target_os, &target_env)
}

fn system_libcpp_of(target_os: &str, target_env: &str) -> Option<&'static str> {
    if target_env == "msvc" {
        None
    } else if target_os == "macos" {
        Some("c++")
    } else {
        Some("stdc++")
    }
}

/// On macOS, add Homebrew's lib directory to rustc's native link search paths,
/// when building for macOS too.
#[cfg(target_os = "macos")]
pub fn set_homebrew_library_path() -> ConfigResult<()> {
    if Environment::of_the_build_target()?.os != OperatingSystem::MacOs {
        return Ok(());
    }

    // see https://github.com/mlir-rs/melior/issues/521
    let output = Command::new("brew")
        .arg("--prefix")
        .output()
        .map_err(ConfigError::from)?;

    if !output.status.success() {
        return Err(ConfigError::ToolExit {
            path: "brew --prefix".into(),
            status: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    let prefix = std::str::from_utf8(&output.stdout)?.trim();
    let brew_lib = PathBuf::from(prefix).join("lib");

    if !brew_lib.is_dir() {
        return Err(ConfigError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Homebrew lib directory not found: {}", brew_lib.display()),
        )));
    }

    println!("cargo:rustc-link-search=native={}", brew_lib.display());
    Ok(())
}

/// No-op on non-macOS platforms.
#[cfg(not(target_os = "macos"))]
pub fn set_homebrew_library_path() -> ConfigResult<()> {
    Ok(())
}

/// Run `llvm-config` with a given argument, meant to be used in `build.rs` files.
/// - `prefix_os`: Optional install prefix (usually from an env var).
///   If `None`, we fall back to searching `llvm-config` in PATH.
/// - `argument`: Argument to pass to `llvm-config` (e.g. `--libs`, `--cflags`).
fn llvm_config(prefix_os: Option<&OsString>, argument: &str) -> ConfigResult<String> {
    let prefix = prefix_os
        .as_ref()
        .map(|p| Path::new(p).join("bin"))
        .unwrap_or_default();

    let llvm_config_binary = if cfg!(target_os = "windows") {
        "llvm-config.exe"
    } else {
        "llvm-config"
    };

    // If prefix is empty, fall back to PATH
    let path: PathBuf = if prefix.as_os_str().is_empty() {
        PathBuf::from(llvm_config_binary)
    } else {
        prefix.join(llvm_config_binary)
    };

    // Run llvm-config
    let output = Command::new(&path)
        .arg("--link-static")
        .arg(argument)
        .output()
        .map_err(|e| {
            println!("cargo:warning=Failed to execute `{}` ({})", path.display(), e);

            match e.kind() {
                std::io::ErrorKind::NotFound => {
                    println!("cargo:warning=The computed llvm-config path does not exist or is not executable.");

                    if let Some(val) = prefix_os.as_ref() {
                        println!(
                            "cargo:warning=Computed from prefix='{}' → '{}/bin/{}'",
                            Path::new(val).display(),
                            Path::new(val).display(),
                            llvm_config_binary
                        );
                    } else {
                        println!("cargo:warning=No prefix provided; relying on PATH to find `{llvm_config_binary}`");
                    }

                    println!("cargo:warning=Fixes:");
                    println!("cargo:warning=- Pass the LLVM/MLIR install prefix (the dir that contains `bin/{llvm_config_binary}`)");
                    println!("cargo:warning=- Or ensure `{llvm_config_binary}` is available in PATH");
                }
                _ => {
                    println!(
                        "cargo:warning=I/O error while launching `{}`: {}",
                        path.display(),
                        e
                    );
                }
            }

            ConfigError::IoError(e)
        })?;

    if !output.status.success() {
        let stderr = str::from_utf8(&output.stderr)?.trim().to_owned();
        if !stderr.is_empty() {
            println!("cargo:warning=llvm-config stderr: {stderr}");
        }
        let code = output.status.code().unwrap_or(-1);
        return Err(ConfigError::ToolExit {
            path: path.display().to_string(),
            status: code,
            stderr,
        });
    }

    let stdout = output.stdout;
    Ok(str::from_utf8(&stdout)?.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What `llvm-config --cxxflags` returns for the bundled LLVM.
    fn cxxflags(includedir: &str) -> String {
        format!(
            "-I{includedir} -std=c++17  -D__STDC_CONSTANT_MACROS -D__STDC_FORMAT_MACROS \
             -D__STDC_LIMIT_MACROS -fno-exceptions"
        )
    }

    /// The macOS include dir, which is always under `Library/Application Support`.
    const MACOS_INCLUDEDIR: &str =
        "/Users/someone/Library/Application Support/tracel/tracel-llvm-22.1.4-7/include";

    #[test]
    fn include_dir_with_a_space_stays_one_argument() {
        let args = split_flags(&cxxflags(MACOS_INCLUDEDIR), MACOS_INCLUDEDIR);

        assert_eq!(args[0], OsString::from(format!("-I{MACOS_INCLUDEDIR}")));
        assert!(
            !args
                .iter()
                .any(|a| a == "Support/tracel/tracel-llvm-22.1.4-7/include"),
            "the include path was split: {args:?}"
        );
    }

    #[test]
    fn other_flags_are_kept_after_the_include_flag() {
        let args = split_flags(&cxxflags(MACOS_INCLUDEDIR), MACOS_INCLUDEDIR);

        assert_eq!(
            args[1..],
            [
                "-std=c++17",
                "-D__STDC_CONSTANT_MACROS",
                "-D__STDC_FORMAT_MACROS",
                "-D__STDC_LIMIT_MACROS",
                "-fno-exceptions",
            ]
            .map(OsString::from)
        );
    }

    #[test]
    fn space_free_include_dir_works_the_same() {
        let includedir = "/home/someone/.local/share/tracel/tracel-llvm-22.1.4-7/include";
        let args = split_flags(&cxxflags(includedir), includedir);

        assert_eq!(args[0], OsString::from(format!("-I{includedir}")));
        assert!(args.iter().any(|a| a == "-fno-exceptions"));
    }

    #[test]
    fn a_build_target_names_the_archive_of_its_platform() {
        let linux_arm64 = Environment::of_target("linux", "aarch64").unwrap();
        let windows_x64 = Environment::of_target("windows", "x86_64").unwrap();

        assert_eq!(linux_arm64.filename(), "linux-AArch64.tar.xz");
        assert_eq!(windows_x64.filename(), "windows-x64.tar.xz");
    }

    #[test]
    fn a_target_no_bundle_is_built_for_is_refused() {
        assert!(matches!(
            Environment::of_target("android", "aarch64"),
            Err(ConfigError::UnsupportedSystem)
        ));
        assert!(matches!(
            Environment::of_target("linux", "riscv64"),
            Err(ConfigError::UnsupportedSystem)
        ));
    }

    #[test]
    fn each_platform_installs_into_a_directory_of_its_own() {
        let data_local = Path::new("/home/someone/.local/share");
        let linux = Environment::of_target("linux", "x86_64")
            .unwrap()
            .llvm_path_under(data_local);
        let windows = Environment::of_target("windows", "x86_64")
            .unwrap()
            .llvm_path_under(data_local);

        assert_ne!(linux, windows);
        assert_eq!(
            linux,
            data_local
                .join("tracel")
                .join("linux-x64")
                .join(format!("tracel-llvm-{TRACEL_LLVM_FULL_VERSION}"))
        );
    }

    #[test]
    fn the_cpp_runtime_follows_the_target() {
        assert_eq!(system_libcpp_of("windows", "msvc"), None);
        assert_eq!(system_libcpp_of("macos", ""), Some("c++"));
        assert_eq!(system_libcpp_of("linux", "gnu"), Some("stdc++"));
    }

    #[test]
    fn a_bundle_with_a_manifest_is_queried_without_llvm_config() {
        let prefix = env::temp_dir().join(format!(
            "tracel-llvm-bundler-recorded-{}",
            std::process::id()
        ));
        fs::create_dir_all(&prefix).unwrap();
        LlvmConfigManifest {
            version: "23.1.0".into(),
            libs: vec!["LLVMCore".into(), "LLVMSupport".into()],
            libnames: vec!["LLVMCore.lib".into(), "LLVMSupport.lib".into()],
            system_libs: vec!["psapi".into(), "ntdll".into()],
            cxxflags: vec!["/std:c++17".into(), "/EHs-c-".into()],
            cflags: vec![],
        }
        .write(&prefix)
        .unwrap();
        let prefix_os = prefix.clone().into_os_string();

        let libs = get_libs(Some(&prefix_os));
        let libdir = get_libdir(Some(&prefix_os));
        let cxxflags = get_cxxflags_args(Some(&prefix_os));
        fs::remove_dir_all(&prefix).unwrap();

        assert_eq!(libs.unwrap(), ["LLVMCore", "LLVMSupport"]);
        assert_eq!(libdir.unwrap(), path_string(prefix.join("lib")));
        assert_eq!(
            cxxflags.unwrap(),
            [
                format!("-I{}", prefix.join("include").display()),
                "/std:c++17".into(),
                "/EHs-c-".into(),
            ]
            .map(OsString::from)
        );
    }
}
