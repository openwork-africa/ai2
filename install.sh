#!/bin/sh
set -eu

repo=openwork-africa/ai2
dir=${AI2_INSTALL_DIR:-$HOME/.local/bin}

case "$(uname -s)" in
  Linux) os=unknown-linux-musl ;;
  Darwin) os=apple-darwin ;;
  *) echo "ai2: there is no build for $(uname -s)" >&2; exit 1 ;;
esac
case "$(uname -m)" in
  x86_64 | amd64) arch=x86_64 ;;
  aarch64 | arm64) arch=aarch64 ;;
  *) echo "ai2: there is no build for $(uname -m)" >&2; exit 1 ;;
esac
archive="ai2-$arch-$os.tar.gz"

if [ -n "${AI2_VERSION:-}" ]; then
  base="https://github.com/$repo/releases/download/$AI2_VERSION"
else
  base="https://github.com/$repo/releases/latest/download"
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
curl -fsSL -o "$tmp/$archive" "$base/$archive"
curl -fsSL -o "$tmp/$archive.sha256" "$base/$archive.sha256"

expected=$(cut -d ' ' -f 1 < "$tmp/$archive.sha256")
if command -v sha256sum >/dev/null 2>&1; then
  actual=$(sha256sum "$tmp/$archive" | cut -d ' ' -f 1)
else
  actual=$(shasum -a 256 "$tmp/$archive" | cut -d ' ' -f 1)
fi
if [ "$expected" != "$actual" ]; then
  echo "ai2: the checksum of $archive does not match; nothing was installed" >&2
  exit 1
fi

tar -xzf "$tmp/$archive" -C "$tmp"
mkdir -p "$dir"
cp "$tmp/ai2" "$dir/ai2.new"
chmod 755 "$dir/ai2.new"
mv "$dir/ai2.new" "$dir/ai2"
echo "ai2 installed to $dir/ai2 ($("$dir/ai2" --version))"
case ":$PATH:" in
  *":$dir:"*) ;;
  *) echo "Add $dir to your PATH to run it as ai2." ;;
esac
