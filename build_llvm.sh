#!/bin/bash
set -euo pipefail

# ----------------------------------------------------------------------------
# Args & usage
# ----------------------------------------------------------------------------
if [[ $# -ne 2 ]]; then
  echo "Usage: $0 <version> <release_number>"
  echo "Example: $0 20.1.4 1"
  exit 2
fi

VERSION="$1"
RELEASE_NUMBER="$2"
BRANCH="llvmorg-$VERSION"
PKG_DIR="tracel-llvm-${VERSION}-${RELEASE_NUMBER}"

echo ">>> Using LLVM branch/tag: $BRANCH"
echo ">>> Package install dir will be: $PKG_DIR"

# ----------------------------------------------------------------------------
# Platform detection
# ----------------------------------------------------------------------------
OS_NAME=$(uname -s)
case "$OS_NAME" in
  Linux)  OS="linux" ;;
  Darwin) OS="macos" ;;
  *) echo "!!! Unsupported OS: $OS_NAME"; exit 1 ;;
esac

UNAME_M=$(uname -m)
case "$UNAME_M" in
  x86_64)        ARCH="x64" ;;
  arm64|aarch64) ARCH="AArch64" ;;
  *)             ARCH="$UNAME_M" ;;
esac

PLATFORM="${OS}-${ARCH}"
echo ">>> Detected platform: $PLATFORM"

# ----------------------------------------------------------------------------
# Dependency installation
# ----------------------------------------------------------------------------
install_linux_deps() {
  echo ">>> Installing Linux dependencies..."
  sudo apt-get update -y
  sudo DEBIAN_FRONTEND=noninteractive apt-get install -y \
    build-essential \
    cmake \
    ninja-build \
    xz-utils \
    git \
    python3
  echo ">>> Linux dependencies installed."
}

install_macos_deps() {
  echo ">>> Ensuring Homebrew and macOS dependencies..."
  if ! command -v brew >/dev/null 2>&1; then
    echo ">>> Homebrew not found, installing..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
    if [[ -d /opt/homebrew/bin ]]; then
      eval "$(/opt/homebrew/bin/brew shellenv)"
    elif [[ -d /usr/local/bin ]]; then
      eval "$(/usr/local/bin/brew shellenv || true)"
    fi
  fi
  brew update || true
  brew install cmake ninja xz git || true
  echo ">>> macOS dependencies installed."
}

echo ">>> Installing build dependencies..."
case "$OS" in
  linux) install_linux_deps ;;
  macos) install_macos_deps ;;
esac

# ----------------------------------------------------------------------------
# Workspace setup
# ----------------------------------------------------------------------------
echo ">>> Preparing workspace..."
rm -rf .llvm
mkdir -p .llvm
cd .llvm

echo ">>> Cloning llvm-project..."
git clone https://github.com/llvm/llvm-project.git
cd llvm-project
git checkout "$BRANCH"

echo ">>> Creating build directory..."
rm -rf build
mkdir -p build

# ----------------------------------------------------------------------------
# Configure
# ----------------------------------------------------------------------------
echo ">>> Configuring LLVM..."
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
  -DCMAKE_INSTALL_PREFIX="../${PKG_DIR}"
echo ">>> Configuration complete."

# ----------------------------------------------------------------------------
# Build + install
# ----------------------------------------------------------------------------
echo ">>> Building and installing LLVM..."
ninja -C build install
echo ">>> Build and install complete."

# ----------------------------------------------------------------------------
# Post-install cleanup
# ----------------------------------------------------------------------------
echo ">>> Cleaning install (keeping only llvm-config)..."
cd "../${PKG_DIR}"
mv bin/llvm-config .
rm -rf bin/*
mv llvm-config bin/
cd ..
echo ">>> Cleanup complete."

# ----------------------------------------------------------------------------
# Package
# ----------------------------------------------------------------------------
echo ">>> Creating package ${PLATFORM}.tar.xz with top-level dir '${PKG_DIR}'..."
tar -cJf "${PLATFORM}.tar.xz" "${PKG_DIR}"
echo ">>> Package created: ${PLATFORM}.tar.xz"

echo "=== LLVM build and packaging completed successfully! ==="
