# llvm-bundler-rs

`llvm-bundler-rs` is a Rust crate designed to automatically bundle LLVM and statically link MLIR into your project. By downloading prebuilt LLVM artifacts and configuring the necessary environment variables, it simplifies the setup process for projects that rely on LLVM and MLIR tooling.

## Features

- **Automatic LLVM Bundling**: Downloads and decompresses prebuilt LLVM artifacts (currently using Linux x64 tarballs) from a GitHub release.
- **Static Linking for MLIR**: Configures environment variables to link LLVM and MLIR libraries properly with the correct order by parsing the CMake with a Regex and doing a topological sort.

## Installation

Add `llvm-bundler-rs` to your project's `Cargo.toml` and use it in your `build.rs` to compile in the right order:

```toml
[dev-dependencies]
llvm-bundler-rs = "0.1.0"
```

To set the env variable and download if missing:
```rust
llvm_bundler_rs::bundler::bundle_cache()?;
```

To get the compile order of MLIR .a:
```rust
use llvm_bundler_rs::{dependency_graph::DependencyGraph, topological_sort::TopologicalSort};

let prefix =
    Path::new(&env::var(format!("MLIR_SYS_{LLVM_MAJOR_VERSION}0_PREFIX")).unwrap_or_default())
        .join("lib")
        .join("cmake")
        .join("mlir")
        .join("MLIRTargets.cmake");
let path = DependencyGraph::from_cmake(prefix)?;
let mlirlib = TopologicalSort::get_ordered_list(&path);
```
