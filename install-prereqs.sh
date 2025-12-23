#!/bin/bash
set -euo pipefail

# ----------------------------------------------------------------------------
# Install required tools for building LLVM and generating bindings
# ----------------------------------------------------------------------------

OS_NAME="$(uname -s)"
case "$OS_NAME" in
  Linux)  OS="linux" ;;
  Darwin) OS="macos" ;;
  *) echo "!!! Unsupported OS: $OS_NAME" >&2; exit 1 ;;
esac

install_linux_deps() {
  echo ">>> Installing Linux dependencies..."
  sudo apt-get update -y
  sudo DEBIAN_FRONTEND=noninteractive apt-get install -y \
    build-essential \
    cmake \
    ninja-build \
    xz-utils \
    git \
    jq
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

echo ">>> Done."
