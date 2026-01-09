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

detect_linux_distro() {
  if [[ -r /etc/os-release ]]; then
    # shellcheck disable=SC1091
    . /etc/os-release
    echo "${ID:-unknown}"
  else
    echo "unknown"
  fi
}

install_debian_deps() {
  echo ">>> Installing Debian/Ubuntu dependencies..."
  sudo apt-get update -y
  sudo DEBIAN_FRONTEND=noninteractive apt-get install -y \
       build-essential \
       cmake \
       ninja-build \
       xz-utils \
       git \
       jq \
       pkg-config \
       libssl-dev
  echo ">>> Debian/Ubuntu dependencies installed."
}

install_arch_deps() {
  echo ">>> Installing Arch Linux dependencies..."
  sudo pacman -Syu --noconfirm --needed \
       base-devel \
       cmake \
       ninja \
       xz \
       git \
       jq \
       pkgconf \
       openssl
  echo ">>> Arch Linux dependencies installed."
}

install_linux_deps() {
  local distro
  distro="$(detect_linux_distro)"

  case "$distro" in
    arch|manjaro|endeavouros|garuda)
      install_arch_deps
      ;;
    debian|ubuntu|linuxmint|pop|elementary|kali)
      install_debian_deps
      ;;
    *)
      echo "!!! Unsupported Linux distro: $distro (from /etc/os-release)" >&2
      echo "    Supported: Debian/Ubuntu-based (apt), Arch-based (pacman)" >&2
      exit 1
      ;;
  esac
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

  brew install cmake ninja xz git jq || true
  echo ">>> macOS dependencies installed."
}

echo ">>> Installing build dependencies..."
case "$OS" in
  linux) install_linux_deps ;;
  macos) install_macos_deps ;;
esac

echo ">>> Done."
