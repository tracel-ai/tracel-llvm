// =========================================================
//     Auto-generated binding selector. Do not edit !
// =========================================================

// BEGIN AUTO-GENERATED FEATURE GATED REGION

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod bindings_macos_aarch64;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub use bindings_macos_aarch64::*;

#[cfg(not(any(
    all(target_os = "macos", target_arch = "aarch64")
)))]
compile_error!("No pre-generated MLIR bindings available for this target_os/target_arch combination.");

// END AUTO-GENERATED FEATURE GATED BINDINGS
