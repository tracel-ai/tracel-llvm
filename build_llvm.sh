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
  # Deterministic directory digest:
  # For each regular file under $1, in lexicographic order by relative path:
  #   PATH\n
  #   SIZE\n
  #   BYTES
  # SHA-256 of the concatenation.
  local root="$1"

  (
      cd "$root" || { echo "cannot cd into: $root" >&2; exit 1; }

      # Print relative paths (starting with ./), sort them, then strip the leading "./"
      LC_ALL=C find . -type f -print0 \
          | LC_ALL=C sort -z \
          | while IFS= read -r -d '' rel; do
          rel="${rel#./}"                         # normalize: remove "./"
          printf '%s\n' "$rel"                    # PATH\n

          # byte size (no spaces/newlines)
          size=$(wc -c < "$rel" | tr -d '[:space:]')
          printf '%s\n' "$size"                   # SIZE\n

          # raw bytes
          cat "$rel"
      done
  ) | sha256_stream
}

# ----------------------------------------------------------------------------
# Workspace setup
# ----------------------------------------------------------------------------
echo ">>> Preparing workspace..."
rm -rf .llvm
mkdir -p .llvm
cd .llvm

echo ">>> Cloning llvm-project..."
git clone --depth=1 --branch "$BRANCH" https://github.com/llvm/llvm-project.git
cd llvm-project

echo ">>> Creating build directory..."
rm -rf build
mkdir -p build

# ----------------------------------------------------------------------------
# Configure
# ----------------------------------------------------------------------------
cmake -S llvm -B build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="../${PKG_DIR}" \
  -DBUILD_SHARED_LIBS=OFF \
  -DLLVM_ENABLE_PROJECTS="mlir" \
  -DLLVM_TARGETS_TO_BUILD="host" \
  -DLLVM_INCLUDE_TOOLS=ON \
  -DLLVM_BUILD_TOOLS=OFF \
  -DLLVM_BUILD_TESTS=OFF -DLLVM_INCLUDE_TESTS=OFF \
  -DLLVM_BUILD_EXAMPLES=OFF -DLLVM_INCLUDE_EXAMPLES=OFF \
  -DLLVM_INCLUDE_DOCS=OFF \
  -DLLVM_ENABLE_ZLIB=OFF -DLLVM_ENABLE_LIBXML2=OFF -DLLVM_ENABLE_LIBEDIT=OFF \
  -DLLVM_ENABLE_LTO=OFF -DLLVM_ENABLE_SPHINX=OFF \
  -DLLVM_ENABLE_RTTI=ON

# ----------------------------------------------------------------------------
# Build + install
# ----------------------------------------------------------------------------
echo ">>> Building and installing LLVM..."
ninja -C build llvm-config install
rm -rf ../${PKG_DIR}/bin/*
cp -f "build/bin/llvm-config" "../${PKG_DIR}/bin"
echo ">>> Build and install complete."
cd ..

# ----------------------------------------------------------------------------
# Cleanup
# ----------------------------------------------------------------------------
echo ">>> Cleaning unneeded stuff from install..."

if [[ "$OS" == "macos" ]]; then
  SHLIB_EXT="dylib"
  LIBSUB="lib"
else
  SHLIB_EXT="so"
  # Prefer lib64 when present (some distros/package layouts)
  LIBSUB="lib"
  [[ -d "${PKG_DIR}/lib64" ]] && LIBSUB="lib64"
fi

echo "lib..."
cd "${LIBSUB}"

# Remove dev-only library subdirs
rm -rf libscanbuild libear objects-Release

# Remove MLIR runner/arm utils
for base in \
    libmlir_c_runner_utils \
    libmlir_runner_utils \
    libmlir_async_runtime \
    libmlir_arm_runner_utils \
    libmlir_float16_utils \
    libmlir_arm_sme_abi_stubs
do
    rm -f "${base}.a" 2>/dev/null || true
done

# --- Remove LTO/Remarks shared libs (and versioned on Linux)
for base in libLTO libRemarks; do
  rm -f "${base}.${SHLIB_EXT}" "${base}.${SHLIB_EXT}."* 2>/dev/null || true
done

cd ..

echo "others..."
rm -rf libexec share

echo ">>> Cleanup complete."
cd ..

# ----------------------------------------------------------------------------
# Package
# ----------------------------------------------------------------------------
echo ">>> Creating package ${PLATFORM}.tar.xz with top-level dir '${PKG_DIR}'..."

if [[ "$OS" == "macos" ]]; then
    # Prevent AppleDouble sidecars and xattrs from being archived
    export COPYFILE_DISABLE=1
    tar --no-mac-metadata --no-xattrs -cJf "${PLATFORM}.tar.xz" "${PKG_DIR}" 2>/dev/null \
        || COPYFILE_DISABLE=1 tar -cJf "${PLATFORM}.tar.xz" "${PKG_DIR}"
else
    tar -cJf "${PLATFORM}.tar.xz" "${PKG_DIR}"
fi

echo ">>> Package created: ${PLATFORM}.tar.xz"

# ----------------------------------------------------------------------------
# Checksums (archive + content) and sidecar JSON
# ----------------------------------------------------------------------------
echo ">>> Computing checksums and writing sidecar JSON..."
archive_sha256="$(sha256_file "${PLATFORM}.tar.xz")"
content_sha256="$(content_sha256_dir "${PKG_DIR}")"
created_at_utc="$(TZ=UTC date +%Y-%m-%dT%H:%M:%SZ)"

jq -n \
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
