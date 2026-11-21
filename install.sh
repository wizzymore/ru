#!/bin/sh
# Installs the latest ru release: curl -fsSL https://releases.eptic.ro/ru.sh | sh
# Set RU_INSTALL_DIR to choose where the binary goes (default: ~/.local/bin).
set -eu

repo="wizzymore/ru"
dir="${RU_INSTALL_DIR:-$HOME/.local/bin}"

case "$(uname -s)" in
  Linux) os="unknown-linux-musl" ;;
  Darwin) os="apple-darwin" ;;
  *) echo "ru: unsupported OS $(uname -s); download from https://github.com/$repo/releases" >&2; exit 1 ;;
esac

case "$(uname -m)" in
  x86_64 | amd64) arch="x86_64" ;;
  aarch64 | arm64) arch="aarch64" ;;
  *) echo "ru: unsupported architecture $(uname -m)" >&2; exit 1 ;;
esac

url="https://github.com/$repo/releases/latest/download/ru-$arch-$os.tar.gz"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "Downloading $url"
if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$url" -o "$tmp/ru.tar.gz"
else
  wget -qO "$tmp/ru.tar.gz" "$url"
fi

tar xzf "$tmp/ru.tar.gz" -C "$tmp"
mkdir -p "$dir"
install -m 755 "$tmp/ru" "$dir/ru"
echo "Installed ru to $dir/ru"

case ":$PATH:" in
  *":$dir:"*) ;;
  *) echo "Note: $dir is not in your PATH. Add it, e.g.: export PATH=\"$dir:\$PATH\"" ;;
esac
