#!/usr/bin/env bash
set -euo pipefail

PROFILE="${1:-release}"
TARGETS=("x86_64-unknown-linux-musl")

ensure_target() {
  local target="$1"
  if ! rustup target list --installed | grep -qx "$target"; then
    echo "Installing Rust target $target..."
    rustup target add "$target"
  fi
}

build_target() {
  local target="$1"
  local profile="$2"
  echo "Building netlab-cli for $target ($profile)..."
  cargo build -p cli --bin netlab-cli --target "$target" --profile "$profile"
}

for target in "${TARGETS[@]}"; do
  ensure_target "$target"
done

for target in "${TARGETS[@]}"; do
  build_target "$target" "$PROFILE"
done

echo "Done."
echo "Artifacts:"
for target in "${TARGETS[@]}"; do
  echo "  target/$target/$PROFILE/netlab-cli"
done
