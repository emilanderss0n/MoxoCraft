# Builds every MoxoCraft filter as a PhotoCraft plug-in and copies the .wasm files into dist/.
#
#   ./build.ps1
#
# Then in PhotoCraft: Filter > Plug-ins > Install Plug-in... and pick a file from dist/, or point
# Edit > Preferences > Plug-ins > Additional Plug-ins Folder at dist/.
#
# Every crate in filters/ is a plug-in, so a new filter is picked up without changing this script.
# dist/ is output only: its plug-ins and notices are replaced on every run.
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

$packages = Get-ChildItem filters -Directory | ForEach-Object {
    $manifest = Join-Path $_.FullName 'Cargo.toml'
    $name = [regex]::Match([IO.File]::ReadAllText($manifest), '(?m)^name\s*=\s*"([^"]+)"').Groups[1].Value
    if (-not $name) { throw "can't find the package name in $manifest" }
    $name
}

rustup target add wasm32-unknown-unknown | Out-Null
cargo build --release --target wasm32-unknown-unknown @($packages | ForEach-Object { '-p', $_ })
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# Start afresh, so a plug-in or notice that no longer exists doesn't linger here.
New-Item -ItemType Directory -Force dist | Out-Null
Remove-Item -Force dist/*.wasm
if (Test-Path dist/luts) { Remove-Item -Recurse -Force dist/luts }
foreach ($package in $packages) {
    $wasm = Get-Item "target/wasm32-unknown-unknown/release/$($package.Replace('-', '_')).wasm"
    Copy-Item $wasm.FullName dist/ -Force
    Write-Host ("dist/{0}  ({1} KB)" -f $wasm.Name, [math]::Round($wasm.Length / 1KB))
}

# The LUT plug-ins carry third-party LUTs whose licenses require their notices to travel with
# every copy, so the licenses, sources and changelog go next to the plug-ins. (PhotoCraft only
# loads *.wasm files directly in its plug-ins folder, so the subfolder is ignored there.)
New-Item -ItemType Directory -Force dist/luts | Out-Null
Get-ChildItem luts/*.txt | ForEach-Object { Copy-Item $_.FullName dist/luts/ -Force }
Write-Host "dist/luts/  (LUT licenses, sources and changelog)"
