# Generating Bindings for a New Platform

These are notes on how to generate bindings for a new platform using the `xtask` tools.

## Prerequisites

In addition to the Rust toolchain, a `cargo xtask` sub-command installs the tools needed to generate the bindings. Install it with:

```sh
cargo xtask setup
```

It will install tools such as `cmake`, `ninja`, `git`, etc., necessary for the build process.

## Creating LLVM Toolchain Archive
 
A custom build of LLVM is used to create the platform-specific bindings. Pre-built versions are published as a part of a [GitHub release](https://github.com/tracel-ai/tracel-llvm/releases).  If your platform is not yet supported, build it for your platform and create an archive with `cargo xtask bundle`. The build process will take a while. 

The resulting bundle is placed in `.llvm/{os}-{arch}.tar.xz` (e.g. `.llvm/linux-AArch64.tar.xz`) along with a checksum sidecar file (`.llvm/{os}-{arch}.checksums.json`).

That said, the  [bindings generation process](#bindings) will run this step automatically if the archive for your platform is not found locally or in the GitHub releases.

## Bindings

With the LLVM tooling available, generate the bindings for your platform:

```sh
cargo xtask bindgen
```
## Testing

Before publishing the bindings and LLVM toolchain, test them against CubeCL. The following manual steps are required:

1. Clone the [CubeCL repository](https://github.com/tracel-ai/cubecl) as a sibling of the `tracel-llvm` repository.
2. In the root `cubecl/Cargo.toml`, point the `tracel-llvm` dependency to the local `tracel-llvm` repository, enabling the `mlir-helpers` feature:

   ```toml
   tracel-llvm = { path = "../tracel-llvm/crates/tracel-llvm", features = ["mlir-helpers"] }
   ``` 
3. In `cubecl/crates/cubecl-cpu/Cargo.toml`, point `tracel-llvm-bundler` to the local path of the `tracel-llvm-bundler` crate:

   ```toml
   tracel-llvm-bundler = { path = "../../../tracel-llvm/crates/tracel-llvm-bundler" }
   ```
4. Copy locally built LLVM toolchain archive and checksum sidecar to `~/.cache/tracel/`, and rename them to include the version number so the `cubecl` build can find them; e.g.:
   ```sh
   cp .llvm/linux-AArch64.tar.xz ~/.cache/tracel/tracel-llvm-20.1.4-6-linux-AArch64.tar.xz
   cp .llvm/linux-AArch64.checksums.json ~/.cache/tracel/tracel-llvm-20.1.4-6-linux-AArch64.checksums.json
   ```
5. Finally, in `cubecl/crates/cubecl-cpu`, run the tests.
   ```sh
   cd ../cubecl/crates/cubecl-cpu
   TRACEL_LLVM_BUNDLER_SKIP_CHECKSUM_DOWNLOAD=1 cargo test
   ```
6. In `tracel-llvm` run `cargo xtask check` and fix any reported issues.

7. When everything is in order, submit a PR to `tracel-llvm` with the new bindings. Coordinate with the maintainers to upload the LLVM toolchain archive to the GitHub release. Note: the changes to `cubecl` for testing are temporary and should not be committed.

