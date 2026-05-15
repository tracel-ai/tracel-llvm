# Tracel LLVM
The purpose of this repository is to provide an interface to MLIR from Rust for the CubeCL backend and support only one version at a time that is bundled automatically at compilation.

## Publish a new release

1) Update the LLVM version and release number in file [`config.rs`](./crates/tracel-llvm-bundler/src/config.rs). Update as well all the occurences of the last version in the `Cargo.toml` files. Commit the changes and push the changes to `main`.

2) Run the workflow `Create Release` which whill create a new release with the bundles and new bindings files attached as assets.

3) Run the command `xtask bindings copy-all` to import the generated bindings for each platform. The command will also lint and format the files.

4) Verify that everything builds correctly.

5) Run the command `xtask bindings git-update` to push the imported bindings files and move the release tag to porperly align with the new commit.

6) At last, run the workflow `Publish` to publish the crates on [crates.io](https://crates.io).

## Add a new platform

See dedicated documentation at [docs/gen-bindings.md](docs/gen-bindings.md).

## Third-Party Acknowledgments

This workspace incorporates code from the following external repositories. Each retains its original license and copyright notices, as detailed in the [COPYRIGHT](COPYRIGHT) file:

- [mlir-rs/mlir-sys](https://github.com/mlir-rs/mlir-sys) - Licensed under MIT
- [mlir-rs/tblgen-rs](https://github.com/mlir-rs/tblgen-rs) - Licensed under MIT or Apache-2.0
- [mlir-rs/mlir_rs](https://github.com/mlir-rs/mlir_rs) - Licensed under Apache-2.0

Please refer to the [COPYRIGHT](COPYRIGHT) file for full license texts and copyright information.
