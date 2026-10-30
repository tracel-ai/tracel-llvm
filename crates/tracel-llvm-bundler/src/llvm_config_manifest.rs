//! What `llvm-config` answers for a bundle, recorded inside the bundle itself.
//!
//! `llvm-config` is an executable of the platform the bundle was built for, so it
//! cannot run on a machine cross-compiling for that platform: a Linux container
//! building for Windows or macOS, or an arm64 machine building for x86-64. The
//! answers are recorded when the bundle is packaged, on its own platform, and read
//! back from there by every query of [`crate::config`].
//!
//! Paths are not recorded: the library and include directories are always `lib`
//! and `include` under wherever the bundle is installed.

use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::config::{ConfigError, ConfigResult};

/// The manifest's file name, at the root of the bundle.
pub const LLVM_CONFIG_MANIFEST_FILE_NAME: &str = "llvm-config.json";

/// The answers of `llvm-config --link-static`, with the install prefix taken out.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlvmConfigManifest {
    /// `--version`.
    pub version: String,
    /// `--libs`, as bare library names in dependency order.
    pub libs: Vec<String>,
    /// `--libnames`, as `llvm-config` prints them on the bundle's platform.
    pub libnames: Vec<String>,
    /// `--system-libs`, as bare library names.
    pub system_libs: Vec<String>,
    /// `--cxxflags`, one argument each, without the include directory.
    pub cxxflags: Vec<String>,
    /// `--cflags`, one argument each, without the include directory.
    pub cflags: Vec<String>,
}

impl LlvmConfigManifest {
    /// The manifest of the bundle installed at `prefix`, or `None` for a bundle
    /// packaged without one.
    pub fn read(prefix: &Path) -> ConfigResult<Option<Self>> {
        match fs::read_to_string(Self::path(prefix)) {
            Ok(text) => serde_json::from_str(&text)
                .map(Some)
                .map_err(ConfigError::Manifest),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(ConfigError::IoError(error)),
        }
    }

    /// Writes the manifest at the root of the bundle installed at `prefix`.
    pub fn write(&self, prefix: &Path) -> ConfigResult<()> {
        let text = serde_json::to_string_pretty(self).map_err(ConfigError::Manifest)?;
        fs::write(Self::path(prefix), text)?;
        Ok(())
    }

    fn path(prefix: &Path) -> PathBuf {
        prefix.join(LLVM_CONFIG_MANIFEST_FILE_NAME)
    }
}
