#!/bin/sh
set -e

REPO="Ranjeesh0701/contract"
INSTALL_DIR="/usr/local/bin"

# Detect OS
OS="$(uname -s)"
case "$OS" in
  Darwin)
    OS_TYPE="darwin"
    ;;
  Linux)
    OS_TYPE="linux"
    ;;
  *)
    echo "Unsupported operating system: $OS. If you are on Windows, use PowerShell."
    exit 1
    ;;
esac

# Detect Architecture
ARCH="$(uname -m)"
case "$ARCH" in
  x86_64)
    ARCH_TYPE="x64"
    ;;
  arm64|aarch64)
    ARCH_TYPE="arm64"
    ;;
  *)
    echo "Unsupported CPU architecture: $ARCH"
    exit 1
    ;;
esac

ARCHIVE_NAME="contract-${OS_TYPE}-${ARCH_TYPE}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/latest/download/${ARCHIVE_NAME}"

echo "==> Detected system: ${OS_TYPE}-${ARCH_TYPE}"
echo "==> Downloading latest Contract release from GitHub..."

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/$ARCHIVE_NAME"
elif command -v wget >/dev/null 2>&1; then
  wget -qO "$TMP_DIR/$ARCHIVE_NAME" "$DOWNLOAD_URL"
else
  echo "Error: curl or wget is required to download Contract."
  exit 1
fi

echo "==> Extracting $ARCHIVE_NAME..."
tar -xzf "$TMP_DIR/$ARCHIVE_NAME" -C "$TMP_DIR"

# Check install permissions
if [ -w "$INSTALL_DIR" ]; then
  SUDO=""
else
  SUDO="sudo"
  echo "==> Installing to $INSTALL_DIR (requires sudo access)..."
fi

$SUDO mv "$TMP_DIR/contract" "$INSTALL_DIR/contract"
$SUDO chmod +x "$INSTALL_DIR/contract"

echo ""
echo "⚡ Contract installed successfully!"
echo "   CLI: $INSTALL_DIR/contract"
echo ""
echo "Run 'contract --help' to get started."
