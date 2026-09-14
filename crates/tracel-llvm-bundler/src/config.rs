use dirs::data_local_dir;
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const TRACEL_LLVM_CACHE_DIRECTORY_NAME: &str = "tracel";
const TRACEL_LLVM_CACHE_PREFIX: &str = "tracel-llvm";
const TRACEL_LLVM_ARTIFACT_BASE_URL: &str =
    "https://github.com/tracel-ai/tracel-llvm/releases/download";
pub const TRACEL_LLVM_VERSION: &str = "23.1.0";
pub const TRACEL_LLVM_RELEASE_NUMBER: &str = "2";
pub const TRACEL_LLVM_FULL_VERSION: &str =
    constcat::concat!(TRACEL_LLVM_VERSION, "-", TRACEL_LLVM_RELEASE_NUMBER);

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
}

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
}

pub struct Environment {
    pub os: OperatingSystem,
    pub arch: Architecture,
}

impl Environment {
    pub fn current() -> Self {
        Self {
            os: OperatingSystem::current(),
            arch: Architecture::current(),
        }
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

    fn filename(&self) -> &'static str {
        match (&self.os, &self.arch) {
            (OperatingSystem::Linux, Architecture::X86_64) => "linux-x64.tar.xz",
            (OperatingSystem::Linux, Architecture::AArch64) => "linux-AArch64.tar.xz",
            (OperatingSystem::MacOs, Architecture::X86_64) => "macos-x64.tar.xz",
            (OperatingSystem::MacOs, Architecture::AArch64) => "macos-AArch64.tar.xz",
            (OperatingSystem::Windows, Architecture::X86_64) => "windows-x64.tar.xz",
            (OperatingSystem::Windows, Architecture::AArch64) => "windows-AArch64.tar.xz",
        }
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

pub fn llvm_path() -> ConfigResult<PathBuf> {
    let directory = format!("{TRACEL_LLVM_CACHE_PREFIX}-{TRACEL_LLVM_FULL_VERSION}");
    data_local_dir()
        .map(|p| p.join(TRACEL_LLVM_CACHE_DIRECTORY_NAME).join(directory))
        .ok_or(ConfigError::UnsupportedSystem)
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
    let libs = llvm_config(prefix_os, "--libs")?;
    Ok(libs.split_whitespace().filter_map(norm_lib).collect())
}

/// Returns a vector of all library names required by LLVM (bare names).
pub fn get_libnames(prefix_os: Option<&OsString>) -> ConfigResult<Vec<String>> {
    let libs = llvm_config(prefix_os, "--libnames")?;
    Ok(libs.split_whitespace().filter_map(norm_lib).collect())
}

/// Returns a vector of all system libraries required by LLVM (bare names).
pub fn get_system_libs(prefix_os: Option<&OsString>) -> ConfigResult<Vec<String>> {
    let libs = llvm_config(prefix_os, "--system-libs")?;
    Ok(libs.split_whitespace().filter_map(norm_lib).collect())
}

/// Returns the lib directory path
pub fn get_libdir(prefix_os: Option<&OsString>) -> ConfigResult<String> {
    let libdir = llvm_config(prefix_os, "--libdir")?;
    Ok(libdir)
}

/// Returns the includes directory path.
pub fn get_includedir(prefix_os: Option<&OsString>) -> ConfigResult<String> {
    let includedir = llvm_config(prefix_os, "--includedir")?;
    Ok(includedir)
}

/// Returns the LLVM version.
pub fn get_version(prefix_os: Option<&OsString>) -> ConfigResult<String> {
    let version = llvm_config(prefix_os, "--version")?;
    Ok(version)
}

/// Returns the CXX flags
pub fn get_cxxflags(prefix_os: Option<&OsString>) -> ConfigResult<String> {
    let flags = llvm_config(prefix_os, "--cxxflags")?;
    Ok(flags)
}

/// Returns the C flags with some tweaks for portability
pub fn get_cflags(prefix_os: Option<&OsString>) -> ConfigResult<String> {
    let flags = llvm_config(prefix_os, "--cflags")?;
    Ok(flags)
}

/// Returns the CXX flags as individual compiler arguments.
///
/// Prefer this over [`get_cxxflags`], whose raw string cannot be split on
/// whitespace: see [`split_flags`].
pub fn get_cxxflags_args(prefix_os: Option<&OsString>) -> ConfigResult<Vec<OsString>> {
    let flags = get_cxxflags(prefix_os)?;
    Ok(split_flags(&flags, &get_includedir(prefix_os)?))
}

/// Returns the C flags as individual compiler arguments.
///
/// Prefer this over [`get_cflags`], whose raw string cannot be split on
/// whitespace: see [`split_flags`].
pub fn get_cflags_args(prefix_os: Option<&OsString>) -> ConfigResult<Vec<OsString>> {
    let flags = get_cflags(prefix_os)?;
    Ok(split_flags(&flags, &get_includedir(prefix_os)?))
}

/// Splits a raw `llvm-config` flag string into individual compiler arguments.
fn split_flags(flags: &str, includedir: &str) -> Vec<OsString> {
    let include_flag = format!("-I{includedir}");
    let mut args = vec![OsString::from(&include_flag)];

    args.extend(
        flags
            .replace(&include_flag, " ")
            .split_whitespace()
            .map(OsString::from),
    );

    args
}

pub fn get_system_libcpp() -> Option<&'static str> {
    if cfg!(target_env = "msvc") {
        None
    } else if cfg!(target_os = "macos") {
        Some("c++")
    } else {
        Some("stdc++")
    }
}

/// On macOS, add Homebrew's lib directory to rustc's native link search paths.
#[cfg(target_os = "macos")]
pub fn set_homebrew_library_path() -> ConfigResult<()> {
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
}
