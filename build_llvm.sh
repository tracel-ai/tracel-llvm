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
# Deps (adds jq for JSON, and uses native sha tools)
# ----------------------------------------------------------------------------
install_linux_deps() {
  echo ">>> Installing Linux dependencies..."
  sudo apt-get update -y
  sudo DEBIAN_FRONTEND=noninteractive apt-get install -y \
    build-essential cmake ninja-build xz-utils git jq
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
  brew install cmake ninja xz git jq || true
  echo ">>> macOS dependencies installed."
}

echo ">>> Installing build dependencies..."
case "$OS" in
  linux) install_linux_deps ;;
  macos) install_macos_deps ;;
esac

# ----------------------------------------------------------------------------
# Hash helpers (portable SHA-256)
# ----------------------------------------------------------------------------
sha256_file() {
  # prints lowercase hex digest of a file
  local f="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$f" | awk '{print tolower($1)}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$f" | awk '{print tolower($1)}'
  elif command -v openssl >/dev/null 2>&1; then
    # openssl prints "HASH  filename" with -r
    openssl dgst -sha256 -r "$f" | awk '{print tolower($1)}'
  else
    echo "No SHA-256 tool found" >&2; exit 1
  fi
}

sha256_stream() {
  # reads stdin, prints lowercase hex digest
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum | awk '{print tolower($1)}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 | awk '{print tolower($1)}'
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 -r | awk '{print tolower($1)}'
  else
    echo "No SHA-256 tool found" >&2; exit 1
  fi
}

content_sha256_dir() {
  # Deterministic directory digest: for each regular file under $1, in
  # sorted (C-locale) order by relative path, feed:
  #   PATH\n
  #   SIZE\n
  #   BYTES
  # into a single SHA-256 stream.
  local root="$1"
  local root_trim="${root%/}"

  LC_ALL=C find "$root_trim" -type f -print0 \
    | LC_ALL=C sort -z \
    | {
        # Binary-safe loop
        while IFS= read -r -d '' f; do
          # Relative path (with forward slashes)
          rel="${f#${root_trim}/}"
          size=$(wc -c < "$f" | tr -d '[:space:]')

          # Emit metadata
          printf '%s\n' "$rel"
          printf '%s\n' "$size"

          # Emit bytes
          cat "$f"
        done
      } | sha256_stream
}

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
echo ">>> Cleaning install..."
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

# ----------------------------------------------------------------------------
# Checksums (archive + content) and sidecar JSON
# ----------------------------------------------------------------------------
echo ">>> Computing checksums and writing sidecar JSON..."
archive_sha256="$(sha256_file "${PLATFORM}.tar.xz")"
content_sha256="$(content_sha256_dir "${PKG_DIR}")"
created_at_utc="$(TZ=UTC date +%Y-%m-%dT%H:%M:%SZ)"

jq -n \
  --arg name "$PKG_DIR" \
  --arg version "$VERSION" \
  --arg release_number "$RELEASE_NUMBER" \
  --arg platform "$PLATFORM" \
  --arg created_at_utc "$created_at_utc" \
  --arg archive_sha256 "$archive_sha256" \
  --arg content_sha256 "$content_sha256" \
  '
  {
    version: $version,
    release_number: $release_number,
    platform: $platform,
    created_at_utc: $created_at_utc,
    archive_sha256: $archive_sha256,
    content_sha256: $content_sha256
  }' > "${PLATFORM}.checksums.json"

echo "Archive sha256: $archive_sha256"
echo "Content sha256: $content_sha256"
echo "Wrote sidecar:  ${PLATFORM}.checksums.json"

echo "=== LLVM build, packaging, and checksum manifest completed successfully! ==="
echo "Workspace: $(pwd)"
echo "Install dir: ${PKG_DIR}"
echo "Archive:     ${PLATFORM}.tar.xz"
echo "Sidecar:     ${PLATFORM}.checksums.json"
