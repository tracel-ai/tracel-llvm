#!/bin/bash

set -euo pipefail

# ----------------------
# Parameters
# ----------------------
VERSION="${1:-21.1.0-rc3}"

# Normalize to llvmorg-... tag
if [[ "$VERSION" =~ ^llvmorg- ]]; then
  BRANCH="$VERSION"
else
  BRANCH="llvmorg-$VERSION"
fi

echo "Using LLVM branch/tag: $BRANCH"

# ----------------------
# Workspace setup
# ----------------------
mkdir -p .llvm
cd .llvm

rm -rf llvm llvm-project

git clone https://github.com/llvm/llvm-project.git
cd llvm-project
git checkout "$BRANCH"

rm -rf build
mkdir -p build

# ----------------------
# Platform detection
# ----------------------
OS_NAME=$(uname -s)
if [[ "$OS_NAME" == "Linux" ]]; then
  OS="linux"
  ARCH="x64"
elif [[ "$OS_NAME" == "Darwin" ]]; then
  OS="macos"
  ARCH="AArch64"
else
  echo "Unsupported OS: $OS_NAME. On Windows use the dedicated 'build_llvm.ps1' script."
  exit 1
fi

PLATFORM="${OS}-${ARCH}"

# ----------------------
# CMake configure
# ----------------------
EXTRA_CMAKE_FLAGS="-DCMAKE_CXX_FLAGS=-Wa,-mbig-obj -DCMAKE_C_FLAGS=-Wa,-mbig-obj"

cmake -S llvm -B build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_SHARED_LIBS=OFF \
  -DLLVM_ENABLE_PROJECTS="clang;mlir" \
  -DLLVM_TARGETS_TO_BUILD="host" \
  -DLLVM_BUILD_TESTS=OFF \
  -DLLVM_INCLUDE_TESTS=OFF \
  -DLLVM_BUILD_EXAMPLES=OFF \
  -DLLVM_INCLUDE_EXAMPLES=OFF \
  -DLLVM_BUILD_DOCS=OFF \
  -DLLVM_ENABLE_DOXYGEN=OFF \
  -DLLVM_ENABLE_LTO=OFF \
  -DLLVM_ENABLE_SPHINX=OFF \
  -DLLVM_STATIC_LINK_CXX_STDLIB=ON \
  -DLLVM_ENABLE_ZLIB=OFF \
  -DLLVM_ENABLE_LIBXML2=OFF \
  -DLLVM_ENABLE_LIBEDIT=OFF \
  -DCMAKE_INSTALL_PREFIX=../llvm $EXTRA_CMAKE_FLAGS

ninja -C build install

# ----------------------
# Post-install cleanup
# ----------------------
cd ../llvm
CONFIG="llvm-config"
mv bin/$CONFIG .
rm bin/*
mv $CONFIG bin/
cd ..

# ----------------------
# Package
# ----------------------
tar -cJf "$PLATFORM.tar.xz" llvm

echo "LLVM build and packaging completed successfully!"
