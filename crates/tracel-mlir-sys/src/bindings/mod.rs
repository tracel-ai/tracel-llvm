// =========================================================
//     Auto-generated binding selector. Do not edit !
// =========================================================

// BEGIN AUTO-GENERATED FEATURE GATED REGION

#[cfg(all(
    not(feature = "xtask"),
    all(target_os = "linux", target_arch = "aarch64")
))]
mod bindings_linux_aarch64;

#[cfg(all(
    not(feature = "xtask"),
    all(target_os = "linux", target_arch = "aarch64")
))]
pub use bindings_linux_aarch64::*;

#[cfg(all(
    not(feature = "xtask"),
    all(target_os = "linux", target_arch = "x86_64")
))]
mod bindings_linux_x86_64;

#[cfg(all(
    not(feature = "xtask"),
    all(target_os = "linux", target_arch = "x86_64")
))]
pub use bindings_linux_x86_64::*;

#[cfg(all(
    not(feature = "xtask"),
    all(target_os = "macos", target_arch = "aarch64")
))]
mod bindings_macos_aarch64;

#[cfg(all(
    not(feature = "xtask"),
    all(target_os = "macos", target_arch = "aarch64")
))]
pub use bindings_macos_aarch64::*;

#[cfg(all(
    not(feature = "xtask"),
    all(target_os = "windows", target_arch = "x86_64")
))]
mod bindings_windows_x86_64;

#[cfg(all(
    not(feature = "xtask"),
    all(target_os = "windows", target_arch = "x86_64")
))]
pub use bindings_windows_x86_64::*;

#[cfg(all(
    not(feature = "xtask"),
    not(any(
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64"),
    )),
))]
compile_error!("No pre-generated bindings available for this target_os/target_arch combination.");

// END AUTO-GENERATED FEATURE GATED BINDINGS
