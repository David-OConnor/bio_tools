#Requires -Version 7
<#
.SYNOPSIS
    Publishes bio_tools to crates.io and athanor_bio_tools to PyPI.

.DESCRIPTION
    Bumps the version, syncs it across the Rust and Python manifests, copies README.md to
    python/PYPI_README.md, commits and pushes, then publishes both packages.

    Everything that can fail is validated before anything irreversible happens: the crate is
    packaged with `cargo publish --dry-run` and the wheel and sdist are built up front, so a
    broken build never leaves one registry published and the other not.

.EXAMPLE
    ./publish.ps1
    Bumps the patch version and publishes.

.EXAMPLE
    ./publish.ps1 -Bump minor
    ./publish.ps1 -Version 1.0.0
    ./publish.ps1 -DryRun
#>
[CmdletBinding()]
param(
    # Which component to increment. Ignored when -Version is given.
    [ValidateSet('patch', 'minor', 'major')]
    [string] $Bump = 'patch',

    # An explicit version, e.g. "1.0.0", instead of bumping.
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string] $Version,

    # Validate and build everything, but make no commits and publish nothing.
    [switch] $DryRun,

    # Publish without the confirmation prompt.
    [switch] $Yes,

    # Publish only one side.
    [switch] $SkipRust,
    [switch] $SkipPython
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = $PSScriptRoot
$pythonDir = Join-Path $root 'python'
$rootManifest = Join-Path $root 'Cargo.toml'
$pythonManifest = Join-Path $pythonDir 'Cargo.toml'
$pyproject = Join-Path $pythonDir 'pyproject.toml'
$readme = Join-Path $root 'README.md'
$pypiReadme = Join-Path $pythonDir 'PYPI_README.md'
$distDir = Join-Path $pythonDir 'dist'

function Write-Step {
    param([Parameter(Mandatory)][string] $Message)
    Write-Host ''
    Write-Host "==> $Message" -ForegroundColor Cyan
}

function Invoke-Native {
    param(
        [Parameter(Mandatory)][string] $Description,
        [Parameter(Mandatory)][string] $Program,
        [string[]] $Arguments = @(),
        [string] $WorkingDirectory = $root
    )
    Write-Step $Description
    Write-Host "    $Program $($Arguments -join ' ')" -ForegroundColor DarkGray
    Push-Location $WorkingDirectory
    try {
        & $Program @Arguments
        if ($LASTEXITCODE -ne 0) {
            throw "$Description failed (exit code $LASTEXITCODE)."
        }
    }
    finally {
        Pop-Location
    }
}

# The `version = "x.y.z"` at the start of a line, which is the [package] one; dependency
# versions live inside inline tables and are therefore never at column 0.
$versionPattern = '(?m)^version\s*=\s*"(?<version>\d+\.\d+\.\d+)"'

function Get-ManifestVersion {
    param([Parameter(Mandatory)][string] $Path)
    $match = [regex]::Match((Get-Content -Raw -LiteralPath $Path), $versionPattern)
    if (-not $match.Success) {
        throw "Could not find a package version in $Path."
    }
    return $match.Groups['version'].Value
}

function Set-ManifestVersion {
    param(
        [Parameter(Mandatory)][string] $Path,
        [Parameter(Mandatory)][string] $NewVersion
    )
    $content = Get-Content -Raw -LiteralPath $Path
    $match = [regex]::Match($content, $versionPattern)
    if (-not $match.Success) {
        return $false
    }
    if ($match.Groups['version'].Value -eq $NewVersion) {
        return $false
    }
    $updated = $content.Remove($match.Index, $match.Length).Insert($match.Index, "version = `"$NewVersion`"")
    Set-Content -LiteralPath $Path -Value $updated -NoNewline
    return $true
}

function Step-Version {
    param(
        [Parameter(Mandatory)][string] $Current,
        [Parameter(Mandatory)][string] $Component
    )
    $parts = $Current.Split('.') | ForEach-Object { [int] $_ }
    switch ($Component) {
        'major' { return "$($parts[0] + 1).0.0" }
        'minor' { return "$($parts[0]).$($parts[1] + 1).0" }
        'patch' { return "$($parts[0]).$($parts[1]).$($parts[2] + 1)" }
    }
}

foreach ($tool in @('git', 'cargo', 'uv')) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        throw "$tool is not on PATH; it is required to publish."
    }
}

# Credentials are checked now rather than at upload time, so a missing PyPI token cannot
# strand us with the crate published and the wheel not. A missing token is asked for once
# and saved to the user's environment, so later runs find it already there.
if (-not $DryRun) {
    if (-not $SkipRust) {
        $cargoCredentials = Join-Path $HOME '.cargo/credentials.toml'
        if (-not $env:CARGO_REGISTRY_TOKEN -and -not (Test-Path $cargoCredentials)) {
            Write-Step 'crates.io login needed'
            Write-Host '    Get a token at https://crates.io/settings/tokens' -ForegroundColor DarkGray
            & cargo login
            if ($LASTEXITCODE -ne 0 -or -not (Test-Path $cargoCredentials)) {
                throw 'crates.io login did not complete.'
            }
        }
    }
    if (-not $SkipPython) {
        $pypirc = Join-Path $HOME '.pypirc'
        if (-not $env:UV_PUBLISH_TOKEN) {
            # A User-scope variable set on an earlier run is not in this process if the shell
            # predates it, so read it back explicitly before asking again.
            $saved = [Environment]::GetEnvironmentVariable('UV_PUBLISH_TOKEN', 'User')
            if ($saved) { $env:UV_PUBLISH_TOKEN = $saved }
        }
        if (-not $env:UV_PUBLISH_TOKEN -and -not (Test-Path $pypirc)) {
            Write-Step 'PyPI login needed'
            Write-Host '    Create an API token at https://pypi.org/manage/account/token/' -ForegroundColor DarkGray
            Write-Host '    (scope it to athanor-bio-tools, or "Entire account" for the first upload)' -ForegroundColor DarkGray
            Write-Host ''
            $token = (Read-Host '    Paste the token (starts with pypi-)').Trim()
            if ($token -notlike 'pypi-*') {
                throw 'That does not look like a PyPI API token; it should start with "pypi-".'
            }
            $env:UV_PUBLISH_TOKEN = $token
            [Environment]::SetEnvironmentVariable('UV_PUBLISH_TOKEN', $token, 'User')
            Write-Host '    Saved for future runs; you will not be asked again.' -ForegroundColor DarkGray
        }
    }
}

$currentVersion = Get-ManifestVersion -Path $rootManifest
$newVersion = if ($Version) { $Version } else { Step-Version -Current $currentVersion -Component $Bump }

Write-Host ''
Write-Host "bio_tools           $currentVersion -> $newVersion  (crates.io)" -ForegroundColor Green
Write-Host "athanor_bio_tools   $currentVersion -> $newVersion  (PyPI)" -ForegroundColor Green
if ($SkipRust) { Write-Host 'Skipping crates.io.' -ForegroundColor Yellow }
if ($SkipPython) { Write-Host 'Skipping PyPI.' -ForegroundColor Yellow }
if ($DryRun) { Write-Host 'Dry run: nothing will be committed, pushed, or published.' -ForegroundColor Yellow }

if (-not $DryRun -and -not $Yes) {
    $answer = Read-Host 'Publish? Releases cannot be undone [y/N]'
    if ($answer -notin @('y', 'Y', 'yes')) {
        Write-Host 'Aborted; no files were changed.' -ForegroundColor Yellow
        exit 1
    }
}

# --- Version and docs sync -------------------------------------------------------------

Write-Step "Setting version to $newVersion"
if (Set-ManifestVersion -Path $rootManifest -NewVersion $newVersion) {
    Write-Host "    Cargo.toml"
}
# maturin reads the wheel version from this manifest, since pyproject.toml declares
# `dynamic = ["version"]`.
if (Set-ManifestVersion -Path $pythonManifest -NewVersion $newVersion) {
    Write-Host "    python/Cargo.toml"
}
# A no-op while the version stays dynamic; keeps working if it is ever pinned here.
if (Set-ManifestVersion -Path $pyproject -NewVersion $newVersion) {
    Write-Host "    python/pyproject.toml"
}

Write-Step 'Copying README.md to python/PYPI_README.md'
Copy-Item -LiteralPath $readme -Destination $pypiReadme -Force

# Refresh both lockfiles so the committed versions match the new package version.
Invoke-Native -Description 'Refreshing Cargo.lock' -Program 'cargo' -Arguments @('check', '--quiet', '--all-targets')
Invoke-Native -Description 'Refreshing python/Cargo.lock' -Program 'cargo' -Arguments @('check', '--quiet') -WorkingDirectory $pythonDir

# --- Validate before anything irreversible ---------------------------------------------

if (-not $SkipRust) {
    Invoke-Native -Description 'Packaging the crate (dry run)' -Program 'cargo' `
        -Arguments @('publish', '--dry-run', '--allow-dirty')
}

if (-not $SkipPython) {
    # Stale artifacts here would be re-uploaded by `uv publish` and rejected by PyPI.
    if (Test-Path $distDir) {
        Write-Step 'Clearing python/dist'
        Remove-Item -Recurse -Force -LiteralPath $distDir
    }
    # A wheel for this platform, plus an sdist so other platforms can build from source.
    Invoke-Native -Description 'Building the wheel and sdist' -Program 'uv' `
        -Arguments @('build', '--wheel', '--sdist') -WorkingDirectory $pythonDir
}

if ($DryRun) {
    Write-Host ''
    Write-Host "Dry run complete. Version is set to $newVersion and artifacts are in python/dist." -ForegroundColor Green
    Write-Host 'Revert with: git checkout -- Cargo.toml Cargo.lock python/' -ForegroundColor DarkGray
    exit 0
}

# --- Commit and push -------------------------------------------------------------------

Invoke-Native -Description 'Staging changes' -Program 'git' -Arguments @('add', '-A')
Invoke-Native -Description 'Committing' -Program 'git' -Arguments @('commit', '-m', "publish $newVersion")
Invoke-Native -Description 'Pushing' -Program 'git' -Arguments @('push')

# --- Publish ---------------------------------------------------------------------------

if (-not $SkipRust) {
    Invoke-Native -Description 'Publishing to crates.io' -Program 'cargo' -Arguments @('publish')
}

if (-not $SkipPython) {
    Invoke-Native -Description 'Publishing to PyPI' -Program 'uv' -Arguments @('publish') `
        -WorkingDirectory $pythonDir
}

Write-Host ''
Write-Host "Published $newVersion." -ForegroundColor Green
if (-not $SkipRust) { Write-Host '    https://crates.io/crates/bio_tools' }
if (-not $SkipPython) { Write-Host '    https://pypi.org/project/athanor-bio-tools' }
Write-Host ''
Write-Host 'Note: the wheel is built for this platform only. Other platforms install from the' -ForegroundColor DarkGray
Write-Host 'sdist and need a Rust toolchain, until CI builds wheels for each target.' -ForegroundColor DarkGray
