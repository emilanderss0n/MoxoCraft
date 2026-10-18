# Builds every MoxoCraft filter as a PhotoCraft plug-in and copies the .wasm files into dist/.
#
#   ./build.ps1
#
# Then in PhotoCraft: Filter > Plug-ins > Install Plug-in... and pick a file from dist/, or point
# Edit > Preferences > Plug-ins > Additional Plug-ins Folder at dist/.
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

rustup target add wasm32-unknown-unknown | Out-Null
cargo build --release --target wasm32-unknown-unknown --workspace --exclude moxocraft-sdk --exclude moxocraft-lut-core --exclude moxocraft-bench --exclude moxocraft-lut-preview
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

New-Item -ItemType Directory -Force dist | Out-Null
Get-ChildItem target/wasm32-unknown-unknown/release/moxocraft_*.wasm | ForEach-Object {
    Copy-Item $_.FullName dist/ -Force
    Write-Host ("dist/{0}  ({1} KB)" -f $_.Name, [math]::Round($_.Length / 1KB))
}

# The LUT plug-ins carry third-party LUTs whose licences require their notices to travel with
# every copy, so the licences, sources and changelog go next to the plug-ins. (PhotoCraft only
# loads *.wasm files directly in its plug-ins folder, so the subfolder is ignored there.)
# Start afresh, so a notice removed from luts/ doesn't linger here.
if (Test-Path dist/luts) { Remove-Item -Recurse -Force dist/luts }
New-Item -ItemType Directory -Force dist/luts | Out-Null
Get-ChildItem luts/*.txt | ForEach-Object { Copy-Item $_.FullName dist/luts/ -Force }
Write-Host "dist/luts/  (LUT licences, sources and changelog)"
