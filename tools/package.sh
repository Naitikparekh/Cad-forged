#!/usr/bin/env bash
# Build release packages of the desktop app into dist/.
#   tools/package.sh                 release build, then the packages for this platform
#   tools/package.sh --no-build      package the existing target/release binary
#
# Linux:   dist/cadforge-<version>-linux-<arch>.tar.gz and dist/cadforge_<version>_<debarch>.deb (when dpkg-deb exists)
# macOS:   dist/cadforge-<version>-macos-<arch>.tar.gz
# Windows: dist/cadforge-<version>-windows-<arch>.zip   (run from Git Bash / MSYS)
set -euo pipefail
cd "$(dirname "$0")/.."

version=$(sed -n '/^\[workspace.package\]/,/^\[/{s/^version = "\(.*\)"/\1/p}' Cargo.toml | head -1)
[ -n "$version" ] || { echo "cannot read the version from Cargo.toml" >&2; exit 1; }
arch=$(uname -m); [ "$arch" = "arm64" ] && arch=aarch64
case "$(uname -s)" in
  Linux*) os=linux; exe=cadforge ;;
  Darwin*) os=macos; exe=cadforge ;;
  MINGW*|MSYS*|CYGWIN*) os=windows; exe=cadforge.exe ;;
  *) echo "unsupported platform $(uname -s)" >&2; exit 1 ;;
esac

if [ "${1:-}" != "--no-build" ]; then
  cargo build --release -p cadforge-app --bin cadforge
fi
bin="target/release/$exe"
[ -f "$bin" ] || { echo "$bin not found; build first" >&2; exit 1; }

name="cadforge-$version-$os-$arch"
rm -rf dist && mkdir -p "dist/$name"
cp "$bin" README.md CSG-LICENSE.txt "dist/$name/"
(cd dist
 if [ "$os" = windows ]; then
   if command -v zip >/dev/null; then zip -qr "$name.zip" "$name"; else powershell -NoProfile -Command "Compress-Archive -Path '$name' -DestinationPath '$name.zip'"; fi
 else
   tar -czf "$name.tar.gz" "$name"
 fi)
rm -rf "dist/$name"

if [ "$os" = linux ] && command -v dpkg-deb >/dev/null; then
  case "$arch" in x86_64) debarch=amd64 ;; aarch64) debarch=arm64 ;; *) debarch=$arch ;; esac
  root="dist/deb-root"
  install -Dm755 "$bin" "$root/usr/bin/cadforge"
  install -Dm644 tools/package/cadforge.desktop "$root/usr/share/applications/cadforge.desktop"
  install -Dm644 README.md "$root/usr/share/doc/cadforge/README.md"
  install -Dm644 CSG-LICENSE.txt "$root/usr/share/doc/cadforge/CSG-LICENSE.txt"
  mkdir -p "$root/DEBIAN"
  cat > "$root/DEBIAN/control" <<CONTROL
Package: cadforge
Version: $version
Section: graphics
Priority: optional
Architecture: $debarch
Maintainer: CadForge maintainers <noreply@users.noreply.github.com>
Depends: libc6, libgl1, libxkbcommon0
Description: 2D drafting and 3D solid modelling with an AutoCAD-style interface
 CadForge has an AutoCAD-style ribbon, command line and command set, DXF
 import and export, layouts, hatching, blocks, AutoLISP and plotting to PDF.
CONTROL
  dpkg-deb --root-owner-group --build "$root" "dist/cadforge_${version}_${debarch}.deb" >/dev/null
  rm -rf "$root"
fi
ls -la dist
