#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
export MACOSX_DEPLOYMENT_TARGET=11.0
binary=slk
mkdir -p dist
for spec in aarch64-apple-darwin:arm64 x86_64-apple-darwin:x86_64; do
  target=${spec%:*}
  arch=${spec#*:}
  rustup target add "$target"
  cargo build --locked --release --target "$target"
  stage=$(mktemp -d "${TMPDIR:-/tmp}/slk-package.XXXXXX")
  trap 'rm -rf "$stage"' EXIT HUP INT TERM
  install -m 755 "target/$target/release/$binary" "$stage/$binary"
  codesign --force --sign - "$stage/$binary"
  cp README.md LICENSE "$stage/"
  COPYFILE_DISABLE=1 tar -czf "dist/$binary-macos-$arch.tar.gz" -C "$stage" "$binary" README.md LICENSE
  rm -rf "$stage"
  trap - EXIT HUP INT TERM
done
(cd dist && shasum -a 256 "$binary-macos-arm64.tar.gz" "$binary-macos-x86_64.tar.gz" > SHA256SUMS)
