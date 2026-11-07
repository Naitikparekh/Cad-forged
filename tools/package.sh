#!/usr/bin/env bash
# Build release packages of the desktop app into dist/.
#   tools/package.sh                 release build, then the packages for this platform
#   tools/package.sh --no-build      package the existing target/release binary
#
# Linux:   dist/cadforge-<version>-linux-<arch>.tar.gz and dist/cadforge_<version>_<debarch>.deb (when dpkg-deb exists)
# macOS:   dist/CadForge-<version>-macos.dmg and dist/cadforge-<version>-macos-universal.zip: CadForge.app, one program
#          for Apple Silicon and Intel Macs, with the logo as its icon, signed ad hoc (not notarized)
# Windows: dist/cadforge-<version>-windows-<arch>.zip   (run from Git Bash / MSYS)
set -euo pipefail
cd "$(dirname "$0")/.."

# (awk, not sed: BSD sed on macOS rejects the usual one-liner)
version=$(awk -F'"' '/^\[workspace.package\]/{f=1;next} /^\[/{f=0} f && /^version *=/{print $2; exit}' Cargo.toml)
[ -n "$version" ] || { echo "cannot read the version from Cargo.toml" >&2; exit 1; }
arch=$(uname -m); [ "$arch" = "arm64" ] && arch=aarch64
case "$(uname -s)" in
  Linux*) os=linux; exe=cadforge ;;
  Darwin*) os=macos; exe=cadforge ;;
  MINGW*|MSYS*|CYGWIN*) os=windows; exe=cadforge.exe ;;
  *) echo "unsupported platform $(uname -s)" >&2; exit 1 ;;
esac

if [ "$os" = macos ]; then
  # ---- macOS: a universal CadForge.app in a .dmg and a .zip
  if [ "${1:-}" != "--no-build" ]; then
    rustup target add aarch64-apple-darwin x86_64-apple-darwin
    for t in aarch64-apple-darwin x86_64-apple-darwin; do
      MACOSX_DEPLOYMENT_TARGET=11.0 cargo build --release -p cadforge-app --bin cadforge --target "$t"
    done
  fi
  rm -rf dist && mkdir -p dist
  app="dist/CadForge.app"
  mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
  lipo -create -output "$app/Contents/MacOS/cadforge" target/aarch64-apple-darwin/release/cadforge target/x86_64-apple-darwin/release/cadforge
  cp crates/cadforge-app/assets/logo/cadforge.icns "$app/Contents/Resources/cadforge.icns"
  cp README.md CSG-LICENSE.txt crates/cadforge-app/assets/fonts/LICENSE-Liberation.txt "$app/Contents/Resources/"
  cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>CadForge</string>
  <key>CFBundleDisplayName</key><string>CadForge</string>
  <key>CFBundleIdentifier</key><string>org.cadforge.CadForge</string>
  <key>CFBundleExecutable</key><string>cadforge</string>
  <key>CFBundleIconFile</key><string>cadforge</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.graphics-design</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
</dict>
</plist>
PLIST
  plutil -lint "$app/Contents/Info.plist"
  # ad hoc signature: Apple Silicon refuses to run unsigned code (this is not a Developer ID: Gatekeeper still asks)
  codesign --force --deep --sign - "$app"
  codesign --verify --deep --strict "$app"
  (cd dist && ditto -c -k --keepParent CadForge.app "cadforge-$version-macos-universal.zip")
  stage="dist/dmg"
  mkdir -p "$stage" && cp -R "$app" "$stage/" && ln -s /Applications "$stage/Applications"
  hdiutil create -volname CadForge -srcfolder "$stage" -ov -format UDZO "dist/CadForge-$version-macos.dmg"
  rm -rf "$stage" "$app"
  ls -la dist
  exit 0
fi

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
   # Git Bash has no zip: Python's zipfile writes forward-slash paths everywhere
   py=$(command -v python3 || command -v python)
   "$py" -m zipfile -c "$name.zip" "$name"
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
Depends: libc6, libgl1, libxkbcommon0, libxkbcommon-x11-0
Description: 2D drafting and 3D solid modelling with an AutoCAD-style interface
 CadForge has an AutoCAD-style ribbon, command line and command set, DXF
 import and export, layouts, hatching, blocks, AutoLISP and plotting to PDF.
CONTROL
  dpkg-deb --root-owner-group --build "$root" "dist/cadforge_${version}_${debarch}.deb" >/dev/null
  rm -rf "$root"
fi
ls -la dist
