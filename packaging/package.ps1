<#
.SYNOPSIS
Packages a release build of Thumbring.

.DESCRIPTION
Makes, in the output folder (dist by default):
  thumbring-<version>-windows-x64.zip   portable: unzip and run
  thumbring-<version>-windows-x64.msi   installer for the current user
  SHA256SUMS                            checksums of the two files above
  release-notes.md                      this version's section of CHANGELOG.md

Run it from the repository root, after building both programs:
  cargo build --release --locked --target x86_64-pc-windows-msvc -p thumbring -p thumbring-settings
The release workflow (.github/workflows/release.yml) runs it the same way.

The license notices need cargo-about and the .msi needs WiX Toolset 5; for a
quick try without them, use -SkipNotices and -SkipMsi.

.EXAMPLE
pwsh packaging/package.ps1 -SkipNotices -SkipMsi
#>
[CmdletBinding()]
param(
    [string]$Target = 'x86_64-pc-windows-msvc',
    [string]$OutDir = 'dist',
    # The project page, for the installer and the release notes.
    [string]$RepoUrl = 'https://github.com/iglesiaskun/thumbring',
    [switch]$SkipNotices,
    [switch]$SkipMsi
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Invoke-Tool {
    # Runs a program and stops if it fails (PowerShell only stops on cmdlet
    # errors, not on exit codes).
    param([string]$Name, [string[]]$Arguments)
    & $Name @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Name failed with exit code $LASTEXITCODE." }
}

$metadata = Invoke-Tool cargo @('metadata', '--format-version', '1', '--no-deps') | ConvertFrom-Json
$version = ($metadata.packages | Where-Object name -eq 'thumbring').version
$name = "thumbring-$version-windows-x64"
$binDir = "target/$Target/release"
$exes = @('thumbring.exe', 'thumbring-settings.exe') | ForEach-Object { Join-Path $binDir $_ }
foreach ($exe in $exes) {
    if (-not (Test-Path $exe)) {
        throw "$exe does not exist. Build it first: cargo build --release --locked --target $Target -p thumbring -p thumbring-settings"
    }
}

if (Test-Path $OutDir) { Remove-Item -Recurse -Force $OutDir }
$stage = Join-Path $OutDir $name
New-Item -ItemType Directory -Force $stage | Out-Null

# Release notes: the section "## [<version>] - <date>" of CHANGELOG.md, up to
# the next section or the link definitions at the end.
$changelog = Get-Content CHANGELOG.md -Raw
$pattern = '(?ms)^## \[' + [regex]::Escape($version) + '\][^\n]*\n(.*?)(?=^## \[|^\[[^\]]+\]: |\z)'
$section = [regex]::Match($changelog, $pattern)
if (-not $section.Success -or -not $section.Groups[1].Value.Trim()) {
    throw "CHANGELOG.md has no section for $version. Add it before releasing."
}
$repo = $RepoUrl -replace '^https://github.com/', ''
$notes = $section.Groups[1].Value.Trim() + @"


---

The ``.msi`` installs Thumbring for the current user, with no administrator
rights; the ``.zip`` runs without installing. The files are not signed yet, so
Windows may warn before running them: see "Known limitations" in the README.

To check a download, compare ``Get-FileHash <file>`` with ``SHA256SUMS``, or
verify where it was built with ``gh attestation verify <file> --repo $repo``.
"@
[IO.File]::WriteAllText((Join-Path (Resolve-Path $OutDir) 'release-notes.md'), $notes + "`n")

# Licenses of the crates compiled into the executable.
$notices = Join-Path $stage 'THIRD-PARTY-NOTICES.html'
if ($SkipNotices) {
    Write-Warning 'Skipping the third-party notices (-SkipNotices): not for a real release.'
    Set-Content -Path $notices -Value '<p>Not generated in this build.</p>'
} else {
    Invoke-Tool cargo @('about', 'generate', '--workspace', '--config', 'packaging/about.toml',
        '--output-file', $notices, 'packaging/about.hbs')
}

# Portable .zip: one folder with the programs and their licenses.
Copy-Item -Destination $stage -Path ($exes + @('README.md', 'LICENSE-MIT', 'LICENSE-APACHE',
    'engine/assets/icons/LICENSE-LUCIDE'))
Compress-Archive -Path $stage -DestinationPath (Join-Path $OutDir "$name.zip")

# Installer. The icon is the one engine/build.rs drew for this build.
if ($SkipMsi) {
    Write-Warning 'Skipping the .msi (-SkipMsi).'
} else {
    $icon = Get-ChildItem "$binDir/build/thumbring-*/out/thumbring.ico" |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if (-not $icon) { throw "The app icon is missing from $binDir/build: build the release first." }
    Invoke-Tool wix @('build', 'packaging/thumbring.wxs', '-arch', 'x64',
        '-ext', 'WixToolset.Util.wixext',
        '-d', "Version=$version", '-d', "BinDir=$binDir", '-d', "Icon=$($icon.FullName)",
        '-d', "Notices=$notices", '-d', "RepoUrl=$RepoUrl",
        '-o', (Join-Path $OutDir "$name.msi"))
    # Debug information of the installer build, not part of the release.
    Remove-Item (Join-Path $OutDir "$name.wixpdb") -ErrorAction SilentlyContinue
}
Remove-Item -Recurse -Force $stage

# Checksums in the format of sha256sum, with Unix line endings, so
# `sha256sum -c SHA256SUMS` works too.
$sums = Get-ChildItem $OutDir -File | Where-Object Extension -in '.zip', '.msi' | Sort-Object Name |
    ForEach-Object { '{0}  {1}' -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower(), $_.Name }
[IO.File]::WriteAllText((Join-Path (Resolve-Path $OutDir) 'SHA256SUMS'), (($sums -join "`n") + "`n"))

Get-ChildItem $OutDir | Format-Table Name, Length -AutoSize
