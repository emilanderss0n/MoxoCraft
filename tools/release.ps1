# Turns a version number into a published GitHub release (see docs/development.md, Releasing).
#
#   ./tools/release.ps1 -Version x.y.z -NotesFile ../notes.md -DryRun
#   ./tools/release.ps1 -Version x.y.z -NotesFile ../notes.md
#
# It checks the repo and the version numbers, runs the tests, builds the plug-ins fresh, packages
# the zip, writes SHA256SUMS.txt, creates the release with gh, then downloads it again and checks
# every file. -DryRun does everything except creating the release and prints where the files are.
# Any failure stops the script with a message saying what to fix.
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$NotesFile,
    [switch]$DryRun
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem

# The git blob hash of spektrafilm's license as published upstream. Its CC BY-SA 4.0 terms need the
# license to travel unchanged with the LUTs, so the copy in the zip must still have this hash.
$LicenseBlobHash = '303ba46793ae40c11442d6b94609165325570a91'
# The plug-ins that ship. build.ps1 builds every crate in filters/; the release stops if what it
# built and this list don't match, so a new filter is shipped (or left out) on purpose.
$Plugins = 'moxocraft_chromatic_aberration.wasm', 'moxocraft_floating_particles.wasm', 'moxocraft_lut.wasm', 'moxocraft_lut_cinematic.wasm', 'moxocraft_lut_creative.wasm', 'moxocraft_snow.wasm'
# The LUTs' licenses, sources and changelog (every .txt file directly in luts/), which their
# licenses require to travel with the plug-ins.
$LutNotices = 'SPEKTRAFILM_LICENSE.txt', 'CC-BY-SA-4.0.txt', 'CC-BY-4.0.txt', 'STRIPED_PURPLE_LICENSE.txt', 'SOURCES.txt', 'CHANGELOG.txt'
# The Chromatic Aberration guide and its pictures, shipped at the same paths so the guide works
# from an unzipped copy. The release stops if the guide links to a file that isn't in the zip. The
# README and its pictures aren't shipped: people read them on GitHub.
$Docs = 'docs/chromatic-aberration.md', 'docs/images/chromatic-aberration/subtle-lens.jpg', 'docs/images/chromatic-aberration/purple-fringing.jpg', 'docs/images/chromatic-aberration/dreamy-spectral.jpg', 'docs/images/chromatic-aberration/rgb-glitch.jpg'

# Where a relative link in the file $From leads, as a path from the repo's root (which is also the
# zip's folder), or $null if it leads out of it.
function Resolve-ZipLink([string]$From, [string]$Link) {
    $parts = New-Object System.Collections.Generic.List[string]
    foreach ($part in ((Split-Path $From -Parent).Replace('\', '/') + '/' + $Link).Split('/')) {
        if ($part -eq '' -or $part -eq '.') { continue }
        if ($part -eq '..') {
            if ($parts.Count -eq 0) { return $null }
            $parts.RemoveAt($parts.Count - 1)
            continue
        }
        $parts.Add($part)
    }
    $parts -join '/'
}

function Fail([string]$Message) {
    Write-Host ''
    Write-Host "Release stopped: $Message" -ForegroundColor Red
    exit 1
}

function Step([string]$Message) {
    Write-Host ''
    Write-Host "== $Message" -ForegroundColor Cyan
}

# Runs a program and returns its exit code and everything it printed, without showing it. Windows
# PowerShell turns a program's error output into errors once it is captured, so that is switched
# off here.
function Invoke-Captured([string]$Exe, [string[]]$Arguments) {
    $ErrorActionPreference = 'Continue'
    $output = & $Exe @Arguments 2>&1 | ForEach-Object { "$_" }
    [pscustomobject]@{ Code = $LASTEXITCODE; Output = ($output -join "`n").Trim() }
}

function Invoke-Git([string[]]$Arguments) {
    $result = Invoke-Captured git $Arguments
    if ($result.Code -ne 0) { Fail "git $($Arguments -join ' ') failed:`n$($result.Output)" }
    $result.Output
}

function Get-Sha256([string]$Path) {
    (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant()
}

function Get-GitBlobHash([byte[]]$Bytes) {
    $header = [Text.Encoding]::ASCII.GetBytes("blob $($Bytes.Length)`0")
    $sha1 = [Security.Cryptography.SHA1]::Create()
    $sha1.TransformBlock($header, 0, $header.Length, $null, 0) | Out-Null
    $sha1.TransformFinalBlock($Bytes, 0, $Bytes.Length) | Out-Null
    -join ($sha1.Hash | ForEach-Object { $_.ToString('x2') })
}

# Checks every file named in SHA256SUMS.txt in $Folder, and that the file itself is in the form
# `sha256sum -c` accepts (LF line endings only).
function Test-Checksums([string]$Folder, [string[]]$Expected) {
    $sumsPath = Join-Path $Folder 'SHA256SUMS.txt'
    if (-not (Test-Path -LiteralPath $sumsPath)) { Fail "SHA256SUMS.txt is missing from $Folder" }
    $bytes = [IO.File]::ReadAllBytes($sumsPath)
    if ($bytes -contains 13) { Fail "SHA256SUMS.txt in $Folder has Windows (CRLF) line endings; sha256sum -c would reject it" }
    $text = [Text.Encoding]::UTF8.GetString($bytes)
    if (-not $text.EndsWith("`n")) { Fail "SHA256SUMS.txt in $Folder doesn't end with a newline" }
    $listed = @()
    foreach ($line in $text.TrimEnd("`n").Split("`n")) {
        if ($line -notmatch '^([0-9a-f]{64})  (.+)$') { Fail "SHA256SUMS.txt in $Folder has a malformed line: $line" }
        $hash, $name = $Matches[1], $Matches[2]
        $file = Join-Path $Folder $name
        if (-not (Test-Path -LiteralPath $file)) { Fail "$name is listed in SHA256SUMS.txt but missing from $Folder" }
        if ((Get-Sha256 $file) -ne $hash) { Fail "$name in $Folder doesn't match its checksum" }
        Write-Host "  OK  $name"
        $listed += $name
    }
    $missing = $Expected | Where-Object { $listed -notcontains $_ }
    if ($missing) { Fail "SHA256SUMS.txt doesn't list: $($missing -join ', ')" }
    $extra = Get-ChildItem -LiteralPath $Folder -File | Where-Object { $_.Name -ne 'SHA256SUMS.txt' -and $listed -notcontains $_.Name }
    if ($extra) { Fail "$Folder has files SHA256SUMS.txt doesn't cover: $(($extra | ForEach-Object Name) -join ', ')" }
}

$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if (-not (Test-Path -LiteralPath $NotesFile -PathType Leaf)) { Fail "can't find the notes file '$NotesFile'" }
$NotesFile = (Resolve-Path -LiteralPath $NotesFile).Path
Push-Location $Root
try {
    $Version = $Version.TrimStart('v')
    if ($Version -notmatch '^\d+\.\d+\.\d+$') { Fail "the version should look like 0.2.0, not '$Version'" }
    $Tag = "v$Version"
    $Name = "MoxoCraft-$Tag"

    foreach ($tool in 'git', 'cargo', 'gh') {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { Fail "$tool isn't installed or isn't on PATH" }
    }
    if ((Invoke-Captured gh @('auth', 'status')).Code -ne 0) { Fail "gh isn't logged in to GitHub; run gh auth login" }

    # 1. The release must be built from exactly what is on GitHub's main branch.
    Step 'Checking the repo'
    $dirty = (Invoke-Git @('status', '--porcelain')) -split "`n" | Where-Object { $_ -and $_ -ne '?? .idea/' }
    if ($dirty) { Fail "the working tree has changes; commit or remove them first:`n$($dirty -join "`n")" }
    $branch = Invoke-Git @('rev-parse', '--abbrev-ref', 'HEAD')
    if ($branch -ne 'main') { Fail "you're on '$branch'; releases are made from main" }
    Invoke-Git @('fetch', '--quiet', 'origin') | Out-Null
    $head = Invoke-Git @('rev-parse', 'HEAD')
    $remoteHead = Invoke-Git @('rev-parse', 'origin/main')
    if ($head -ne $remoteHead) { Fail "main ($head) and origin/main ($remoteHead) differ; push or pull first" }
    Write-Host "  main is clean and matches origin/main at $head"

    # 2. The version must be the one asked for. It is set once, in the root Cargo.toml; every
    # plug-in's manifest takes it from there when it is built.
    Step "Checking the version is $Version"
    $versionSources = @(
        @{ File = 'Cargo.toml'; Pattern = '(?m)^version\s*=\s*"([^"]+)"' }
    )
    foreach ($source in $versionSources) {
        $found = [regex]::Matches([IO.File]::ReadAllText((Join-Path $Root $source.File)), $source.Pattern)
        if ($found.Count -ne 1) { Fail "expected one version in $($source.File), found $($found.Count)" }
        $fileVersion = $found[0].Groups[1].Value
        if ($fileVersion -ne $Version) { Fail "$($source.File) says version $fileVersion, not $Version" }
        Write-Host "  $($source.File): $fileVersion"
    }
    if ((Invoke-Captured git @('rev-parse', '--quiet', '--verify', "refs/tags/$Tag")).Code -eq 0) {
        Fail "the tag $Tag already exists here; pick a new version"
    }
    $remoteTag = Invoke-Captured git @('ls-remote', '--tags', 'origin', "refs/tags/$Tag")
    if ($remoteTag.Code -ne 0) { Fail "couldn't check GitHub's tags:`n$($remoteTag.Output)" }
    if ($remoteTag.Output) { Fail "the tag $Tag already exists on GitHub; pick a new version" }
    $existing = Invoke-Captured gh @('release', 'view', $Tag)
    if ($existing.Code -eq 0) { Fail "a GitHub release named $Tag already exists (perhaps a draft); pick a new version" }
    if ($existing.Output -notmatch 'not found') { Fail "couldn't check GitHub's releases:`n$($existing.Output)" }
    Write-Host "  $Tag isn't used yet"

    # 6 (checked early so a typo doesn't waste a build). The notes are rewritten for every release,
    # so they come from a file. They are re-saved as UTF-8 so a file written by Windows
    # PowerShell's > (which saves UTF-16) still reaches GitHub intact.
    Step 'Checking the release notes'
    $notes = [IO.File]::ReadAllText($NotesFile).Replace("`r`n", "`n").Trim()
    if (-not $notes) { Fail "the notes file '$NotesFile' is empty" }
    foreach ($required in 'spektrafilm', 'CC BY-SA 4.0') {
        if ($notes -notmatch [regex]::Escape($required)) { Fail "the notes don't mention '$required'; the film LUTs' license needs their credit (see v0.1.0's notes)" }
    }
    # Windows PowerShell's > misreads gh's output, so a character like the menu arrow comes out as
    # several wrong ones. These are the tell-tale starts of that (Unicode escapes keep this file
    # plain ASCII, which Windows PowerShell needs).
    foreach ($garble in "$([char]0xD4)$([char]0xC7)", "$([char]0xE2)$([char]0x20AC)", "$([char]0xFFFD)") {
        if ($notes.Contains($garble)) { Fail "the notes contain garbled characters ('$garble'), probably from saving gh's output in Windows PowerShell; save them from Git Bash or an editor instead" }
    }
    $otherZips =[regex]::Matches($notes, 'MoxoCraft-v\d+\.\d+\.\d+\.zip') | ForEach-Object Value | Where-Object { $_ -ne "$Name.zip" }
    if ($otherZips) { Fail "the notes mention $(($otherZips | Select-Object -Unique) -join ', ') instead of $Name.zip" }
    Write-Host "  $NotesFile looks right"

    # Checked early too. Relative links and pictures (Markdown, or HTML src, srcset and href) must
    # lead somewhere; links to websites are fine. The guide ships in the zip, so its links must point
    # at files that ship with it. The README doesn't ship, but GitHub shows it at this commit, so
    # its links must point at files committed here.
    Step 'Checking the documentation links'
    $shipped = $Docs + ($LutNotices | ForEach-Object { "luts/$_" })
    foreach ($file in $Docs) {
        if (-not (Test-Path -LiteralPath (Join-Path $Root $file) -PathType Leaf)) { Fail "$file, listed in `$Docs, doesn't exist" }
    }
    $committed = (Invoke-Git @('-c', 'core.quotepath=off', 'ls-files')) -split "`n"
    foreach ($page in @('README.md') + @($Docs | Where-Object { $_ -like '*.md' })) {
        $text = [regex]::Replace([IO.File]::ReadAllText((Join-Path $Root $page)), '(?s)```.*?```', '')
        $targets = @([regex]::Matches($text, '!?\[[^\]]*\]\(([^)\s]+)\)') | ForEach-Object { $_.Groups[1].Value })
        $targets += @([regex]::Matches($text, '(?:src|srcset|href)="([^"\s,]+)') | ForEach-Object { $_.Groups[1].Value })
        foreach ($target in $targets) {
            if ($target -match '^(https?:|mailto:|#)') { continue }
            $path = Resolve-ZipLink $page ($target -split '#')[0]
            if ($page -eq 'README.md') {
                if ($committed -notcontains $path) { Fail "README.md links to $target, which isn't committed in the repo" }
            } elseif ($shipped -notcontains $path) {
                Fail "$page links to $target, which isn't in the zip; add it to `$Docs or link to it on GitHub"
            }
        }
    }
    Write-Host "  README.md only links to committed files, and $(@($Docs | Where-Object { $_ -like '*.md' }).Count) guide(s) only to files in the zip"

    # 3. Test, then build from scratch so nothing stale ends up in the release. --locked stops if
    # Cargo.lock still has the old version, which would otherwise change during the build.
    Step 'Running the tests'
    cargo test --locked
    if ($LASTEXITCODE -ne 0) { Fail "cargo test failed (if it says Cargo.lock needs updating, run cargo build and commit Cargo.lock)" }

    Step 'Building the plug-ins fresh'
    foreach ($old in 'target/wasm32-unknown-unknown', 'dist') {
        if (Test-Path $old) { Remove-Item -Recurse -Force $old }
    }
    & (Join-Path $Root 'build.ps1')
    if ($LASTEXITCODE -ne 0) { Fail 'build.ps1 failed' }
    Set-Location $Root
    foreach ($file in $Plugins) {
        if (-not (Test-Path "dist/$file")) { Fail "build.ps1 didn't produce dist/$file" }
    }
    $unlisted = Get-ChildItem dist -Filter '*.wasm' | ForEach-Object Name | Where-Object { $Plugins -notcontains $_ }
    if ($unlisted) { Fail "build.ps1 built $($unlisted -join ', '), which `$Plugins at the top of this script doesn't list; add it there to ship it" }
    $after = (Invoke-Git @('status', '--porcelain')) -split "`n" | Where-Object { $_ -and $_ -ne '?? .idea/' }
    if ($after) { Fail "the build changed files in the repo, so the release wouldn't match the commit:`n$($after -join "`n")" }

    # 4. Package. The zip holds one folder with the plug-ins, the Chromatic Aberration guide and its
    # pictures, and the LUT notices; the plug-ins are also uploaded on their own.
    Step 'Packaging'
    $work = Join-Path ([IO.Path]::GetTempPath()) ("$Name-" + [IO.Path]::GetRandomFileName().Replace('.', ''))
    $upload = Join-Path $work 'upload'
    New-Item -ItemType Directory -Force $upload | Out-Null

    $notices = Get-ChildItem -LiteralPath (Join-Path $Root 'luts') -Filter '*.txt' -File | ForEach-Object Name
    if (Compare-Object @($notices) @($LutNotices)) { Fail "luts/ holds $($notices -join ', '); update `$LutNotices at the top of this script to match" }

    $contents = [ordered]@{}
    foreach ($file in $Plugins) { $contents["$Name/$file"] = Join-Path $Root "dist/$file" }
    foreach ($file in $LutNotices) { $contents["$Name/luts/$file"] = Join-Path $Root "luts/$file" }
    foreach ($file in $Docs) { $contents["$Name/$file"] = Join-Path $Root $file }

    $licenseSource = Join-Path $Root 'luts/SPEKTRAFILM_LICENSE.txt'
    if ((Get-GitBlobHash ([IO.File]::ReadAllBytes($licenseSource))) -ne $LicenseBlobHash) {
        Fail 'luts/SPEKTRAFILM_LICENSE.txt is no longer identical to the upstream license'
    }

    $zipPath = Join-Path $upload "$Name.zip"
    $zip = [IO.Compression.ZipFile]::Open($zipPath, 'Create')
    try {
        foreach ($entry in $contents.Keys) {
            [IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $contents[$entry], $entry, 'Optimal') | Out-Null
        }
    } finally { $zip.Dispose() }
    foreach ($file in $Plugins) { Copy-Item (Join-Path $Root "dist/$file") $upload }

    # Read the zip back: exactly the expected files, each identical to what went in.
    $zip = [IO.Compression.ZipFile]::OpenRead($zipPath)
    try {
        $names = @($zip.Entries | ForEach-Object FullName)
        if (Compare-Object $names @($contents.Keys)) { Fail "the zip holds the wrong files: $($names -join ', ')" }
        foreach ($entry in $zip.Entries) {
            $stream = $entry.Open()
            $buffer = New-Object IO.MemoryStream
            try { $stream.CopyTo($buffer) } finally { $stream.Dispose() }
            $bytes = $buffer.ToArray()
            $sourceBytes = [IO.File]::ReadAllBytes($contents[$entry.FullName])
            if ((Get-GitBlobHash $bytes) -ne (Get-GitBlobHash $sourceBytes)) { Fail "$($entry.FullName) changed on its way into the zip" }
            if ($entry.FullName -eq "$Name/luts/SPEKTRAFILM_LICENSE.txt" -and (Get-GitBlobHash $bytes) -ne $LicenseBlobHash) {
                Fail 'the license in the zip is not byte-identical to upstream'
            }
            Write-Host ("  {0}  ({1} KB)" -f $entry.FullName, [math]::Round($entry.Length / 1KB))
        }
    } finally { $zip.Dispose() }

    # 5. Checksums, written with LF line endings: v0.1.0 first shipped CRLF ones that
    # `sha256sum -c` rejected.
    Step 'Writing SHA256SUMS.txt'
    $assets = @("$Name.zip") + $Plugins
    $sums = ($assets | ForEach-Object { "$(Get-Sha256 (Join-Path $upload $_))  $_" }) -join "`n"
    $sumsPath = Join-Path $upload 'SHA256SUMS.txt'
    [IO.File]::WriteAllText($sumsPath, "$sums`n", (New-Object Text.UTF8Encoding $false))
    Test-Checksums $upload $assets

    $notesPath = Join-Path $work 'notes.md'
    [IO.File]::WriteAllText($notesPath, "$notes`n", (New-Object Text.UTF8Encoding $false))

    if ($DryRun) {
        Step 'Dry run: no release created'
        Write-Host "  Files that would be uploaded: $upload"
        Write-Host "  Release notes as they would be sent: $notesPath"
        Write-Host "  It would be created as '$Tag' on $head, titled 'MoxoCraft $Tag'."
        return
    }

    # 7. Publish. gh creates the tag on GitHub at the commit that was tested and built.
    Step "Creating the GitHub release $Tag"
    $files = $assets + 'SHA256SUMS.txt' | ForEach-Object { Join-Path $upload $_ }
    gh release create $Tag --target $head --title "MoxoCraft $Tag" --notes-file $notesPath @files
    if ($LASTEXITCODE -ne 0) { Fail "gh release create failed; check GitHub for a half-made release or tag $Tag before trying again" }

    # 8. Download what GitHub now serves and check it against the checksums.
    Step 'Checking the published release'
    $download = Join-Path $work 'download'
    New-Item -ItemType Directory -Force $download | Out-Null
    gh release download $Tag --dir $download
    if ($LASTEXITCODE -ne 0) { Fail "couldn't download $Tag to check it" }
    Test-Checksums $download $assets
    if (Compare-Object ([IO.File]::ReadAllBytes($sumsPath)) ([IO.File]::ReadAllBytes((Join-Path $download 'SHA256SUMS.txt')))) {
        Fail "the published SHA256SUMS.txt differs from the one written here"
    }

    Write-Host ''
    Write-Host "Released MoxoCraft $Tag. Files are in $work" -ForegroundColor Green
    Write-Host 'Run git fetch --tags to get the new tag locally.'
} finally {
    Pop-Location
}
