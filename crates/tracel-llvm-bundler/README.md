# tracel-llvm-bundler

`tracel-llvm-bundler` downloads a prebuilt, self-contained LLVM toolchain and emits the link
configuration for it, so that a project can link LLVM statically without a system installation.

## Features

- **Automatic LLVM bundling**: downloads, verifies and decompresses a prebuilt LLVM release for the
  host platform, cached under the user data directory.
- **Link configuration**: emits the library search path, the LLVM libraries in dependency order, the
  system libraries and the target initialization wrappers.

## Usage

Add it as a build dependency; the bundle is downloaded when the crate itself is compiled, before
your `build.rs` runs:

```toml
[build-dependencies]
tracel-llvm-bundler = "23.1.0-1"
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
