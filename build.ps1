# Bundles shell.html and the source modules into the self-contained CadForge.html (plus the CadForge-Repaired.html copy).
# Usage:  powershell -ExecutionPolicy Bypass -File build.ps1
#         powershell -ExecutionPolicy Bypass -File build.ps1 -Include acad-core,acad-commands -Out _dev\commands.html
# -Include limits the workspace (acad-*) modules; the drafting/mesh engine modules are always bundled.
param([string[]]$Include, [string]$Out)
$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$utf8 = New-Object System.Text.UTF8Encoding($false)
$read = { param($name) [IO.File]::ReadAllText((Join-Path $root $name), $utf8) }
$engine = @('core','dialogs','workspace','advanced','mesh','compatibility','csg-library','booleans','repairs','profiles','sheets')
$workspace = @('acad-core','acad-icons','acad-commands','acad-drafting','acad-interact','acad-views','acad-palettes','acad-shell')
if ($Include) { $Include = $Include | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ }; $workspace = $workspace | Where-Object { $Include -contains $_ } }
$bundle = & $read 'shell.html'
foreach ($name in $engine + $workspace) {
  $path = Join-Path $root "$name.js"
  if (-not (Test-Path $path)) { Write-Warning "Skipping missing module $name.js"; continue }
  $tag = if ($name -eq 'csg-library') { '<script id="cadforge-csg">' } else { "<script data-module=`"$name`">" }
  $source = & $read "$name.js"
  if ($source -match '</script') { throw "$name.js contains a closing script tag" }
  $bundle += $tag + $source + '</script>'
}
$bundle += '</html>'
$targets = if ($Out) { @($Out) } else { @('CadForge.html','CadForge-Repaired.html') }
foreach ($target in $targets) {
  $full = if ([IO.Path]::IsPathRooted($target)) { $target } else { Join-Path $root $target }
  New-Item -ItemType Directory -Force (Split-Path $full) | Out-Null
  [IO.File]::WriteAllText($full, $bundle, $utf8)
  "Built $target ($($bundle.Length) chars)"
}
