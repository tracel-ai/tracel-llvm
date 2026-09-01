# Supporting a New Platform

These are notes on how to build the LLVM toolchain for a new platform using the `xtask` tools, and
how to verify that consumers link against it correctly.

## Prerequisites

Install Tracel xtask CLI:

```sh
cargo install tracel-xtask-cli
```

Then install the dependencies with:

```sh
cargo xtask setup
```

It will install tools such as `cmake`, `ninja`, `git`, etc., necessary for the build process.

## Creating LLVM Toolchain Archive

A custom build of LLVM is used. Pre-built versions are published as a part of a [GitHub release](https://github.com/tracel-ai/tracel-llvm/releases). If your platform is not yet supported, build it for your platform and create an archive with `cargo xtask bundle build`. The build process will take a while.

The resulting bundle is placed in `.llvm/{os}-{arch}.tar.xz` (e.g. `.llvm/linux-AArch64.tar.xz`) along with a checksum sidecar file (`.llvm/{os}-{arch}.checksums.json`).

## How the linking is wired

`llvm-sys` normally resolves LLVM inside its own build script, from `LLVM_SYS_221_PREFIX` or from
`llvm-config` on `PATH`. That build script runs before the build script of any crate that could have
downloaded a bundle first, and Cargo has no way to pass an environment variable back up to an
already-scheduled build script, so the bundle cannot be fed to it.

Instead `llvm-sys` is built with two features that make its build script return immediately, before
it looks for `llvm-config`:

```toml
llvm-sys = { version = "231", features = ["no-llvm-linking", "disable-alltargets-init"] }
```

The consumer then calls [`llvm_sys::link`](../crates/tracel-llvm-bundler/src/llvm_sys.rs) from its
own `build.rs`, which emits everything `llvm-sys` would have emitted:

- the target initialization wrappers, compiled from `wrappers/target.c` against the bundle headers
  (`disable-alltargets-init` means `llvm-sys` no longer builds them, but it still declares the
  `LLVM_Initialize*` symbols);
- the bundle `lib` directory as a link search path;
- the LLVM archives, in the order `llvm-config --link-static --libs` reports them;
- the system libraries from `llvm-config --link-static --system-libs`, plus the C++ standard library.

Adding `tracel-llvm-bundler` as a **build dependency** is what downloads the bundle: it happens when
that crate is compiled, which Cargo schedules before the consumer's build script runs.

## Testing against CubeCL

Before publishing the LLVM toolchain, test it against CubeCL, whose `cubecl-cpu` test suite
exercises the JIT and so covers the runtime side as well as the link. The following manual steps are
required:

1. Clone the [CubeCL repository](https://github.com/tracel-ai/cubecl) as a sibling of the `tracel-llvm` repository.
2. In `cubecl/crates/cubecl-cpu/Cargo.toml`, point `tracel-llvm-bundler` to the local path of the `tracel-llvm-bundler` crate:

   ```toml
   tracel-llvm-bundler = { path = "../../../tracel-llvm/crates/tracel-llvm-bundler" }
   ```

3. Copy locally built LLVM toolchain archive and checksum sidecar to `~/.cache/tracel/`, and rename them to include the version number so the `cubecl` build can find them; e.g.:
   ```sh
   cp .llvm/linux-AArch64.tar.xz ~/.cache/tracel/tracel-llvm-20.1.4-7-linux-AArch64.tar.xz
   cp .llvm/linux-AArch64.checksums.json ~/.cache/tracel/tracel-llvm-20.1.4-7-linux-AArch64.checksums.json
   ```
4. Finally, in `cubecl/crates/cubecl-cpu`, run the tests.
   ```sh
   cd ../cubecl/crates/cubecl-cpu
   TRACEL_LLVM_BUNDLER_SKIP_CHECKSUM_DOWNLOAD=1 cargo test
   ```
5. In `tracel-llvm` run `cargo xtask check` and fix any reported issues.

6. When everything is in order, submit a PR to `tracel-llvm` with the new platform support. Coordinate with the maintainers to upload the LLVM toolchain archive to the GitHub release. Note: the changes to `cubecl` for testing are temporary and should not be committed.

## Troubleshooting

| Symptom                                                                                  | Cause                                                                                                                                                                                                                                                                     |
| ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `No suitable version of LLVM was found system-wide or pointed to by LLVM_SYS_221_PREFIX` | The two `llvm-sys` features are not enabled. They only apply if some crate in the graph depends on `llvm-sys` with them, so that feature unification carries them to the shared instance.                                                                                 |
| `undefined reference to LLVM_InitializeNativeTarget`                                     | `disable-alltargets-init` is on but `link()` was never called, so nothing compiled the wrappers.                                                                                                                                                                          |
| Undefined C++ symbols (`std::__cxx11::…`, `operator new`)                                | The C++ standard library did not make it onto the link line; `link()` emits it via `get_system_libcpp()`.                                                                                                                                                                 |
| Undefined LLVM symbols despite the archives being listed                                 | Link order. Static archives must follow the objects that reference them, and the wrappers must precede the archives; `link()` emits them in that order.                                                                                                                   |
| Builds only on some machines                                                             | Likely `disable-alltargets-init` is missing while `no-llvm-linking` is set. `llvm-sys` then still tries to locate `llvm-config`: it silently skips the wrappers when none is found, and compiles a second copy of them against the other toolchain's headers when one is. |
