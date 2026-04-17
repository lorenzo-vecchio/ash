#!/usr/bin/env sh
# install.sh — install the Ash language toolchain
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/lorenzo-vecchio/ash/main/install.sh | sh
#
# To install a specific version:
#   curl -fsSL .../install.sh | sh -s -- --version v0.2.0
#
# To install to a custom directory:
#   curl -fsSL .../install.sh | sh -s -- --prefix /usr/local
#
# To uninstall:
#   ash --uninstall
#   # or manually: rm $(which ash)

set -e

REPO="lorenzo-vecchio/ash"
BINARY="ash"
DEFAULT_PREFIX="${HOME}/.local"

# ── Helpers ────────────────────────────────────────────────────────────────────

say()  { printf '\033[1;32m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33mwarning:\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

need() {
    command -v "$1" >/dev/null 2>&1 || die "required tool '$1' not found — please install it first"
}

# ── Argument parsing ───────────────────────────────────────────────────────────

VERSION=""
PREFIX=""

while [ $# -gt 0 ]; do
    case "$1" in
        --version) VERSION="$2"; shift 2 ;;
        --prefix)  PREFIX="$2";  shift 2 ;;
        *) die "unknown option: $1" ;;
    esac
done

PREFIX="${PREFIX:-$DEFAULT_PREFIX}"
BIN_DIR="${PREFIX}/bin"

# ── Detect platform ────────────────────────────────────────────────────────────

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux)
        case "$ARCH" in
            x86_64)  ASSET="ash-linux-x86_64" ;;
            *) die "unsupported Linux architecture: $ARCH (only x86_64 is supported)" ;;
        esac ;;
    Darwin)
        case "$ARCH" in
            arm64)   ASSET="ash-macos-arm64" ;;
            x86_64)  ASSET="ash-macos-x86_64" ;;
            *) die "unsupported macOS architecture: $ARCH" ;;
        esac ;;
    *)
        die "unsupported OS: $OS — only Linux and macOS are supported by this script" ;;
esac

# ── Resolve version ────────────────────────────────────────────────────────────

need curl

if [ -z "$VERSION" ]; then
    say "Fetching latest release…"
    VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"
    [ -n "$VERSION" ] || die "could not determine latest version — pass --version explicitly"
fi

say "Installing Ash ${VERSION} for ${OS}/${ARCH}"

# ── Download ───────────────────────────────────────────────────────────────────

ARCHIVE="${ASSET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

say "Downloading ${URL}"
curl -fsSL --progress-bar "$URL" -o "${TMP}/${ARCHIVE}" || \
    die "download failed — check that ${VERSION} exists at https://github.com/${REPO}/releases"

# ── Extract & install ──────────────────────────────────────────────────────────

tar -xzf "${TMP}/${ARCHIVE}" -C "$TMP"
chmod +x "${TMP}/${ASSET}"

mkdir -p "$BIN_DIR"
rm -f "${BIN_DIR}/${BINARY}"
mv "${TMP}/${ASSET}" "${BIN_DIR}/${BINARY}"

say "Installed to ${BIN_DIR}/${BINARY}"

# ── PATH — auto-add to shell profile if needed ─────────────────────────────────

case ":${PATH}:" in
    *":${BIN_DIR}:"*) ;;  # already on PATH, nothing to do
    *)
        # Detect which profile file to write to
        if [ -n "$ZSH_VERSION" ] || [ "$(basename "$SHELL")" = "zsh" ]; then
            PROFILE="$HOME/.zshrc"
        elif [ -n "$BASH_VERSION" ] || [ "$(basename "$SHELL")" = "bash" ]; then
            if [ -f "$HOME/.bash_profile" ]; then
                PROFILE="$HOME/.bash_profile"
            else
                PROFILE="$HOME/.bashrc"
            fi
        else
            PROFILE="$HOME/.profile"
        fi

        LINE="export PATH=\"\$PATH:${BIN_DIR}\""

        # Only append if the line isn't already in the file
        if ! grep -qF "$LINE" "$PROFILE" 2>/dev/null; then
            printf '\n# Ash language toolchain\n%s\n' "$LINE" >> "$PROFILE"
            say "Added ${BIN_DIR} to PATH in ${PROFILE}"
        fi

        # Also export for the current session
        export PATH="$PATH:${BIN_DIR}"
        ;;
esac

# ── Verify ─────────────────────────────────────────────────────────────────────

if command -v ash >/dev/null 2>&1; then
    say "ash $(ash version 2>/dev/null || true) — ready"
fi

echo ""
echo "  Quick start:"
echo "    ash run program.ash     # interpret a file"
echo "    ash repl                # interactive REPL"
echo "    ash --help              # full usage"
echo ""
echo "  To uninstall:"
echo "    rm ${BIN_DIR}/${BINARY}"
echo ""
