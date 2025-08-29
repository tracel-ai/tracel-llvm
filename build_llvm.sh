#!/bin/bash
set -euo pipefail

# ----------------------------------------------------------------------------
# Parameters
# ----------------------------------------------------------------------------
VERSION="${1:-21.1.0-rc3}"
BRANCH="llvmorg-$VERSION"
echo "Using LLVM branch/tag: $BRANCH"

# ----------------------------------------------------------------------------
# Platform detection
# ----------------------------------------------------------------------------
OS_NAME=$(uname -s)
case "$OS_NAME" in
  Linux)  OS="linux" ;;
  Darwin) OS="macos" ;;
  *) echo "Unsupported OS: $OS_NAME"; exit 1 ;;
esac

# Architecture label
UNAME_M=$(uname -m)
case "$UNAME_M" in
  x86_64)           ARCH="x64" ;;
  arm64|aarch64)    ARCH="AArch64" ;;
  *)                ARCH="$UNAME_M" ;;
esac

PLATFORM="${OS}-${ARCH}"

# ----------------------------------------------------------------------------
# Dependency installation
# ----------------------------------------------------------------------------
install_linux_deps() {
  echo "Installing Linux dependencies with apt..."
  sudo apt-get update -y
  sudo DEBIAN_FRONTEND=noninteractive apt-get install -y \
    build-essential \
    cmake \
    ninja-build \
    xz-utils \
    git \
    python3
}

install_macos_deps() {
  echo "Ensuring Homebrew and macOS dependencies..."
  if ! command -v brew >/dev/null 2>&1; then
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    # Add brew to PATH for this shell (common on CI)
    if [[ -d /opt/homebrew/bin ]]; then
      eval "$(/opt/homebrew/bin/brew shellenv)"
    elif [[ -d /usr/local/bin ]]; then
      eval "$(/usr/local/bin/brew shellenv || true)"
    fi
  fi
  brew update || true
  brew install cmake ninja xz git || true
}

case "$OS" in
  linux)  install_linux_deps ;;
  macos)  install_macos_deps ;;
esac

# ----------------------------------------------------------------------------
# Workspace setup
# ----------------------------------------------------------------------------
mkdir -p .llvm
cd .llvm

rm -rf llvm llvm-project

git clone https://github.com/llvm/llvm-project.git
cd llvm-project
git checkout "$BRANCH"

rm -rf build
mkdir -p build

# ----------------------------------------------------------------------------
# Configure
# ----------------------------------------------------------------------------
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
  -DCMAKE_INSTALL_PREFIX=../llvm

# ----------------------------------------------------------------------------
# Build + install
# ----------------------------------------------------------------------------
ninja -C build install

# ----------------------------------------------------------------------------
# Post-install: keep only llvm-config in bin
# ----------------------------------------------------------------------------
cd ../llvm
CONFIG="llvm-config"
mv "bin/$CONFIG" .
rm -rf bin/*
mv "$CONFIG" bin/
cd ..

# ----------------------------------------------------------------------------
# Package (xz tarball)
# ----------------------------------------------------------------------------
tar -cJf "$PLATFORM.tar.xz" llvm

echo "LLVM build and packaging completed successfully! -> $PLATFORM.tar.xz"
