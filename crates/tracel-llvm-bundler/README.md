# tracel-llvm-bundler

`tracel-llvm-bundler` downloads a prebuilt, self-contained LLVM toolchain and emits the link
configuration for it, so that a project can link LLVM statically without a system installation.

## Features

- **Automatic LLVM bundling**: downloads, verifies and decompresses a prebuilt LLVM release for the
  platform being built, cached under the user data directory.
- **Cross-compilation**: the platform is the build script's target, not the machine running it, so
  building for another OS or architecture links that platform's LLVM.
- **Link configuration**: emits the library search path, the LLVM libraries in dependency order, the
  system libraries and the target initialization wrappers.

## Usage

Add it as a build dependency; the bundle is downloaded the first time your `build.rs` asks for it
(through `llvm_path()` or `llvm_sys::link()`), for that build script's target:

```toml
[build-dependencies]
tracel-llvm-bundler = "23.1.0-4"
```

Build `llvm-sys` with `no-llvm-linking` and `disable-alltargets-init` so that it does not look for
`llvm-config` on its own:

```toml
[dependencies]
llvm-sys = { version = "230", features = ["no-llvm-linking", "disable-alltargets-init"] }
```

Then emit the link configuration from `build.rs`:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracel_llvm_bundler::llvm_sys::link()?;
    Ok(())
}
```

Individual `llvm-config` queries are also available from `tracel_llvm_bundler::config`, taking the
install prefix returned by `llvm_path()`:

```rust
let prefix = tracel_llvm_bundler::config::llvm_path()?.into_os_string();
let libdir = tracel_llvm_bundler::config::get_libdir(Some(&prefix))?;
```

## Cross-compiling

`llvm-config` is an executable of the platform a bundle was built for, so a machine cross-compiling
for that platform cannot run it. Each bundle therefore records what `llvm-config` answers in an
`llvm-config.json` at its root, written when it is packaged, and every query reads it from there.
A bundle without it, such as one installed by hand, is still queried by running its `llvm-config`.

The C++ code your `build.rs` compiles against LLVM, and the link, still need a toolchain for the
target: a cross C++ compiler and linker on Linux, and an MSVC toolchain (such as `cargo-xwin`) for
Windows, whose bundles are built with MSVC.
