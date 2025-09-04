# Tracel LLVM
The purpose of this repository is to provide an interface to MLIR from Rust for the CubeCL backend and support only one version at a time that is bundled automatically at compilation.

## Publish a new release

Dispatch the workflow `Release` manually by providing it with the LLVM tag and release number.

The workflow will create a new release named `v{LLVM_TAG}-{RELEASE_NUMBER}`.

**Note:** Currently the workflow does not build the MacOS archive. It must be built manually by executing the script `./build_llvm.sh` at the root of the repository. It will create an archive at `.llvm/macos-AArch64.tar.xz`. Edit the GitHub release and upload this archive to it manually.

## Third-Party Acknowledgments

This workspace incorporates code from the following external repositories. Each retains its original license and copyright notices, as detailed in the [COPYRIGHT](COPYRIGHT) file:

- [mlir-rs/mlir-sys](https://github.com/mlir-rs/mlir-sys) - Licensed under MIT
- [mlir-rs/tblgen-rs](https://github.com/mlir-rs/tblgen-rs) - Licensed under MIT or Apache-2.0
- [mlir-rs/mlir_rs](https://github.com/mlir-rs/mlir_rs) - Licensed under Apache-2.0

Please refer to the [COPYRIGHT](COPYRIGHT) file for full license texts and copyright information.
