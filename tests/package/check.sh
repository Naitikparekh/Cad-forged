#!/usr/bin/env bash
# Checks the packages tools/package.sh left in dist/: the archive holds the program and its notices, and the .deb
# installs the program, a desktop entry and the documentation. Usage: tests/package/check.sh
set -euo pipefail
cd "$(dirname "$0")/../.."
shopt -s nullglob
if [ "$(uname -s)" = Darwin ]; then
  # macOS: the .zip holds CadForge.app with a universal program, its icon and Info.plist; the .dmg mounts with the app
  zips=(dist/cadforge-*-macos-universal.zip); dmgs=(dist/CadForge-*-macos.dmg)
  [ ${#zips[@]} -eq 1 ] && [ ${#dmgs[@]} -eq 1 ] || { echo "expected one .zip and one .dmg in dist/" >&2; exit 1; }
  tmp=$(mktemp -d)
  ditto -x -k "${zips[0]}" "$tmp"
  app="$tmp/CadForge.app"
  for f in Contents/MacOS/cadforge Contents/Info.plist Contents/Resources/cadforge.icns Contents/Resources/README.md; do
    [ -e "$app/$f" ] || { echo "the app lacks $f" >&2; exit 1; }
  done
  archs=$(lipo -archs "$app/Contents/MacOS/cadforge")
  case "$archs" in *arm64*x86_64*|*x86_64*arm64*) ;; *) echo "not universal: $archs" >&2; exit 1 ;; esac
  plutil -lint "$app/Contents/Info.plist"
  codesign --verify --deep --strict "$app"
  hdiutil verify "${dmgs[0]}"
  mnt=$(mktemp -d)
  hdiutil attach -nobrowse -readonly -mountpoint "$mnt" "${dmgs[0]}" >/dev/null
  ok=0; [ -x "$mnt/CadForge.app/Contents/MacOS/cadforge" ] && [ -L "$mnt/Applications" ] && ok=1
  hdiutil detach "$mnt" >/dev/null
  [ $ok -eq 1 ] || { echo "the .dmg lacks CadForge.app or the Applications link" >&2; exit 1; }
  echo "macOS app ok ($archs): ${zips[0]}, ${dmgs[0]}"
  exit 0
fi
archives=(dist/cadforge-*.tar.gz dist/cadforge-*.zip)
[ ${#archives[@]} -eq 1 ] || { echo "expected one archive in dist/, found ${#archives[@]}" >&2; exit 1; }
py=$(command -v python3 || command -v python || true)
# (a function, not a case inside $(...): the bash 3.2 that macOS ships cannot parse that)
list_archive() {
  case "$1" in
    *.zip) "$py" -c 'import sys, zipfile; print("\n".join(zipfile.ZipFile(sys.argv[1]).namelist()))' "$1" ;;
    *) tar -tzf "$1" ;;
  esac
}
list=$(list_archive "${archives[0]}")
for f in README.md CSG-LICENSE.txt; do echo "$list" | grep -q "/$f$" || { echo "archive lacks $f" >&2; exit 1; }; done
echo "$list" | grep -Eq "/cadforge(\.exe)?$" || { echo "archive lacks the program" >&2; exit 1; }
echo "archive ok: ${archives[0]}"
debs=(dist/*.deb)
if [ ${#debs[@]} -gt 0 ]; then
  contents=$(dpkg-deb --contents "${debs[0]}")
  for f in ./usr/bin/cadforge ./usr/share/applications/cadforge.desktop ./usr/share/doc/cadforge/README.md; do
    echo "$contents" | grep -q " $f$" || { echo "deb lacks $f" >&2; exit 1; }
  done
  info=$(dpkg-deb --info "${debs[0]}")
  echo "$info" | grep -q "Package: cadforge" || { echo "deb has no package name" >&2; exit 1; }
  echo "deb ok: ${debs[0]}"
fi
