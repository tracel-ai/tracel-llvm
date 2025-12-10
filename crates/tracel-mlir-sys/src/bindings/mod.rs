// =========================================================
//     Auto-generated binding selector. Do not edit !
// =========================================================

// BEGIN AUTO-GENERATED FEATURE GATED REGION

#[cfg(all(feature = "llvm_20_1_4", target_os = "macos", target_arch = "aarch64"))]
mod bindings_20_1_4_macos_aarch64;

#[cfg(all(feature = "llvm_20_1_4", target_os = "macos", target_arch = "aarch64"))]
pub use bindings_20_1_4_macos_aarch64::*;

#[cfg(not(any(
    all(feature = "llvm_20_1_4", target_os = "macos", target_arch = "aarch64")
)))]
compile_error!("No LLVM bindings found for this LLVM version and target platform.");

// END AUTO-GENERATED FEATURE GATED BINDINGS
