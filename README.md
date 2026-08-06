# Tracel LLVM
The purpose of this repository is to provide a prebuilt, self-contained LLVM toolchain for the CubeCL CPU backend and support only one version at a time that is bundled automatically at compilation.

## Publish a new release

1) Update the LLVM version and release number in file [`config.rs`](./crates/tracel-llvm-bundler/src/config.rs). Update as well all the occurences of the last version in the `Cargo.toml` files. Commit the changes and push the changes to `main`.

2) Run the workflow `Create Release` which whill create a new release with the bundles and their checksums attached as assets.

3) Verify that everything builds correctly.

4) At last, run the workflow `Publish Crates` to publish the crates on [crates.io](https://crates.io).
