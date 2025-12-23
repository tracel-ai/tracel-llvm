use std::env;

use tracel_xtask::prelude::*;

#[derive(Clone, Debug)]
pub struct PlatformTriple {
    pub os: String,
    pub arch: String,
}

impl PlatformTriple {
    pub fn detect() -> anyhow::Result<Self> {
        let os = env::consts::OS.to_string();
        let arch = env::consts::ARCH.to_string();
        Ok(Self { os, arch })
    }

    pub fn archive_stem(&self) -> String {
        let arch = match self.arch.as_str() {
            "x86_64" => "x64",
            "aarch64" => "AArch64",
            other => other,
        };
        let os = match self.os.as_str() {
            "macos" => "macos",
            "linux" => "linux",
            "windows" => "windows",
            other => other,
        };
        format!("{os}-{arch}")
    }
}
