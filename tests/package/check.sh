#!/usr/bin/env bash
# Checks the packages tools/package.sh left in dist/: the archive holds the program and its notices, and the .deb
# installs the program, a desktop entry and the documentation. Usage: tests/package/check.sh
set -euo pipefail
cd "$(dirname "$0")/../.."
shopt -s nullglob
archives=(dist/cadforge-*.tar.gz dist/cadforge-*.zip)
[ ${#archives[@]} -eq 1 ] || { echo "expected one archive in dist/, found ${#archives[@]}" >&2; exit 1; }
py=$(command -v python3 || command -v python || true)
list=$(case "${archives[0]}" in *.zip) "$py" -c 'import sys, zipfile; print("\n".join(zipfile.ZipFile(sys.argv[1]).namelist()))' "${archives[0]}" ;; *) tar -tzf "${archives[0]}" ;; esac)
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
