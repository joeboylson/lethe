#!/bin/sh
# Installs or updates lethe in ~/.local/bin. Two ways to run it:
#
#   curl -fsSL https://raw.githubusercontent.com/joeboylson/lethe/main/install.sh | sh
#       downloads the latest release built for this computer
#   ./install.sh
#       in a copy of the repo: builds it (page included) from the code there
#
# Run it again to update. LETHE_VERSION=v0.1.0 picks a release.
set -e

repo="joeboylson/lethe"
bin_dir="$HOME/.local/bin"
mkdir -p "$bin_dir"
new="$bin_dir/.lethe.new"

# Piped into sh, $0 is the shell, not this file: always download then.
here=""
case "$0" in
  *install.sh) here="$(cd "$(dirname "$0")" && pwd)" ;;
esac
if [ -n "$here" ] && [ -f "$here/Cargo.toml" ] && grep -q '^name = "lethe"' "$here/Cargo.toml"; then
  echo "Building the page..."
  (cd "$here/web" && npm ci --no-fund --no-audit --loglevel=error && npm run build --silent >/dev/null)
  echo "Building lethe..."
  (cd "$here" && cargo build --release --quiet)
  # A copy, not a link, so the installed one keeps working while the repo changes.
  cp "$here/target/release/lethe" "$new"
else
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) target=aarch64-apple-darwin ;;
    Darwin-x86_64) target=x86_64-apple-darwin ;;
    Linux-x86_64) target=x86_64-unknown-linux-musl ;;
    Linux-aarch64 | Linux-arm64) target=aarch64-unknown-linux-musl ;;
    *) echo "No lethe build for $(uname -s) $(uname -m). Build it from the repo: https://github.com/$repo" >&2; exit 1 ;;
  esac
  if [ -n "$LETHE_VERSION" ]; then
    base="https://github.com/$repo/releases/download/$LETHE_VERSION"
  else
    base="https://github.com/$repo/releases/latest/download"
  fi
  echo "Downloading lethe for $target..."
  curl -fsSL "$base/lethe-$target" -o "$new"
  want="$(curl -fsSL "$base/lethe-$target.sha256" | cut -d' ' -f1)"
  if command -v shasum >/dev/null; then got="$(shasum -a 256 "$new" | cut -d' ' -f1)"; else got="$(sha256sum "$new" | cut -d' ' -f1)"; fi
  if [ "$want" != "$got" ]; then
    rm -f "$new"
    echo "The download doesn't match its checksum, so it wasn't installed. Try again." >&2
    exit 1
  fi
  chmod +x "$new"
fi

mv "$new" "$bin_dir/lethe"
echo "Installed $("$bin_dir/lethe" --version) in $bin_dir"

# A running page keeps the old copy until it restarts.
if [ "$(uname -s)" = Darwin ] && [ -f "$HOME/Library/LaunchAgents/dev.lethe.server.plist" ]; then
  launchctl kickstart -k "gui/$(id -u)/dev.lethe.server" >/dev/null 2>&1 && echo "Restarted the page."
fi

case ":$PATH:" in
  *":$bin_dir:"*) ;;
  *) echo "Add this to your shell profile: export PATH=\"$bin_dir:\$PATH\"" ;;
esac
# Keep the installed skills in step with this version.
if [ -f "${CLAUDE_CONFIG_DIR:-$HOME/.claude}/skills/lethe/SKILL.md" ]; then
  "$bin_dir/lethe" setup >/dev/null && echo "Updated the /lethe and /recall skills."
else
  echo "Next: lethe setup, then lethe start"
fi
