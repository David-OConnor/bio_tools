#Requires -Version 7
<#
.SYNOPSIS
    Publishes bio_tools to crates.io, and athanor_bio_tools and bio_tools_app to PyPI.

.DESCRIPTION
    Bumps the version, syncs it across the Rust and Python manifests, copies README.md to
    python/PYPI_README.md and python_cli/PYPI_README.md, commits and pushes, then publishes all
    three packages.

    Everything that can fail is validated before anything irreversible happens: the crate is
    packaged with `cargo publish --dry-run` and every wheel and sdist is built up front, so a
    broken build never leaves one registry published and the other not.

    bio_tools_app ships the compiled executable rather than Python code, so its wheel is
    platform-specific and this run only produces the Windows one. Run ./publish_cli_linux.sh on
    Linux afterwards, from the commit this script pushes, for the Linux wheel.

.EXAMPLE
    ./publish.ps1
    Bumps the patch version and publishes.

.EXAMPLE
    ./publish.ps1 -Bump minor
    ./publish.ps1 -Version 1.0.0
    ./publish.ps1 -DryRun

.EXAMPLE
    ./publish.ps1 -CliOnly
    Publishes just the Windows bio_tools_app wheel, at the version already in Cargo.toml. No
    version bump, no commit, no crates.io, no athanor_bio_tools. This is the Windows counterpart
    to ./publish_cli_linux.sh, and the way to add a wheel to a release that is already out.
#>
# Positional binding is off so that a mistyped switch — `--CliOnly` instead of `-CliOnly`, say —
# reports itself as an unrecognized argument rather than being silently bound to -Bump.
[CmdletBinding(PositionalBinding = $false)]
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

    # Publish only some of the packages.
    [switch] $SkipRust,
    [switch] $SkipPython,
    [switch] $SkipCli,

    # Build and publish only the bio_tools_app wheel for this machine, at the version already in
    # Cargo.toml. Skips the version bump, the git commit, and the other two packages entirely.
    [switch] $CliOnly
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = $PSScriptRoot
$pythonDir = Join-Path $root 'python'
$rootManifest = Join-Path $root 'Cargo.toml'
$pythonManifest = Join-Path $pythonDir 'Cargo.toml'
$pyproject = Join-Path $pythonDir 'pyproject.toml'
$readme = Join-Path $root 'README.md'
$license = Join-Path $root 'LICENSE'
$pypiReadme = Join-Path $pythonDir 'PYPI_README.md'
$distDir = Join-Path $pythonDir 'dist'

# The CLI wheel: the compiled executable, packaged so `pip install bio_tools_app` puts `bio_tools`
# on PATH. It has no manifest of its own; python_cli/pyproject.toml builds the crate in $root.
$cliDir = Join-Path $root 'python_cli'
$cliPypiReadme = Join-Path $cliDir 'PYPI_README.md'
$cliPypiLicense = Join-Path $cliDir 'PYPI_LICENSE'
$cliDistDir = Join-Path $cliDir 'dist'

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

# Asks for a PyPI API token once and saves it to the user's environment, so later runs find it
# already there. Credentials are collected before anything is published, so a missing token cannot
# strand us with some packages released and the rest not.
function Read-PypiToken {
    param(
        [Parameter(Mandatory)][string] $Project,
        [Parameter(Mandatory)][string] $Variable
    )
    Write-Step "PyPI login needed for $Project"
    Write-Host '    Create an API token at https://pypi.org/manage/account/token/' -ForegroundColor DarkGray
    Write-Host "    (scope it to $Project, or `"Entire account`" for the first upload)" -ForegroundColor DarkGray
    Write-Host ''
    # -MaskInput so the token is not echoed into the terminal's scrollback.
    $token = (Read-Host '    Paste the token (starts with pypi-)' -MaskInput).Trim()
    if ($token -notlike 'pypi-*') {
        throw 'That does not look like a PyPI API token; it should start with "pypi-".'
    }
    [Environment]::SetEnvironmentVariable($Variable, $token, 'User')
    Write-Host '    Saved for future runs; you will not be asked again.' -ForegroundColor DarkGray
    return $token
}

# The three parts of a bio_tools_app release, shared by a full run and by -CliOnly.

# bio_tools_app gets its own token variable, because a token scoped to athanor-bio-tools cannot
# upload to bio-tools-app. publish_cli_linux.sh reads the same variable on Linux.
function Resolve-CliToken {
    if (-not $env:BIO_TOOLS_APP_PYPI_TOKEN) {
        # A User-scope variable set on an earlier run is not in this process if the shell predates
        # it, so read it back explicitly before asking again.
        $saved = [Environment]::GetEnvironmentVariable('BIO_TOOLS_APP_PYPI_TOKEN', 'User')
        if ($saved) { $env:BIO_TOOLS_APP_PYPI_TOKEN = $saved }
    }
    if (-not $env:BIO_TOOLS_APP_PYPI_TOKEN) {
        $env:BIO_TOOLS_APP_PYPI_TOKEN = Read-PypiToken -Project 'bio-tools-app' -Variable 'BIO_TOOLS_APP_PYPI_TOKEN'
    }
}

function Build-CliWheel {
    if (Test-Path $cliDistDir) {
        Write-Step 'Clearing python_cli/dist'
        Remove-Item -Recurse -Force -LiteralPath $cliDistDir
    }
    # Wheel only, and no sdist: an sdist would need a Rust toolchain on the installing machine,
    # which is the thing this package exists to avoid. Platforms without a wheel are told to use
    # `cargo install bio_tools` or the Releases page instead.
    Invoke-Native -Description 'Building the CLI wheel' -Program 'uvx' `
        -Arguments @('--from', 'maturin>=1.9,<2.0', 'maturin', 'build', '--release', '--out', 'dist') `
        -WorkingDirectory $cliDir
}

function Publish-CliWheel {
    # Named explicitly rather than left to `uv publish`'s default of everything in dist/, so a
    # stray file could never be uploaded.
    $wheels = @(Get-ChildItem -LiteralPath $cliDistDir -Filter '*.whl' | ForEach-Object { $_.FullName })
    if ($wheels.Count -eq 0) {
        throw "No wheel was produced in $cliDistDir."
    }
    # The token goes through the environment rather than `--token`, so it stays out of the echoed
    # command line and out of this process's arguments. UV_PUBLISH_TOKEN belongs to
    # athanor_bio_tools during a full run, so it is swapped for the duration and put back after.
    $previousToken = $env:UV_PUBLISH_TOKEN
    $env:UV_PUBLISH_TOKEN = $env:BIO_TOOLS_APP_PYPI_TOKEN
    try {
        Invoke-Native -Description 'Publishing bio_tools_app to PyPI' -Program 'uv' `
            -Arguments (@('publish') + $wheels) -WorkingDirectory $cliDir
    }
    finally {
        $env:UV_PUBLISH_TOKEN = $previousToken
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

# --- CLI wheel only --------------------------------------------------------------------
#
# Adds this platform's bio_tools_app wheel to whatever version is already in Cargo.toml, and
# touches nothing else: no bump, no commit, no other package. The Windows counterpart to
# ./publish_cli_linux.sh, and the way to attach a wheel to a release that is already out.
if ($CliOnly) {
    # These only describe a full release, so accepting them here would imply a bump that never
    # happens. Better to say so than to ignore them.
    foreach ($conflict in @('Bump', 'Version', 'SkipRust', 'SkipPython', 'SkipCli')) {
        if ($PSBoundParameters.ContainsKey($conflict)) {
            throw "-$conflict cannot be combined with -CliOnly; -CliOnly publishes the CLI wheel at the version already in Cargo.toml."
        }
    }

    $cliVersion = Get-ManifestVersion -Path $rootManifest

    Write-Host ''
    Write-Host "bio_tools_app   $cliVersion   (PyPI, Windows wheel)" -ForegroundColor Green
    Write-Host 'Nothing else is published, and no version is bumped or committed.' -ForegroundColor DarkGray
    if ($DryRun) { Write-Host 'Dry run: the wheel is built but not published.' -ForegroundColor Yellow }

    # maturin reads the wheel's long description from this copy of the README.
    Copy-Item -LiteralPath $readme -Destination $cliPypiReadme -Force
    Copy-Item -LiteralPath $license -Destination $cliPypiLicense -Force

    Build-CliWheel

    if ($DryRun) {
        Write-Host ''
        Write-Host 'Dry run complete. The wheel is in python_cli/dist.' -ForegroundColor Green
        exit 0
    }

    Resolve-CliToken

    if (-not $Yes) {
        # Braced, because `?` is a legal character in a PowerShell variable name and would
        # otherwise be read as part of it.
        $answer = Read-Host "Publish bio_tools_app ${cliVersion}? Releases cannot be undone [y/N]"
        if ($answer -notin @('y', 'Y', 'yes')) {
            Write-Host 'Aborted; nothing was published.' -ForegroundColor Yellow
            exit 1
        }
    }

    Publish-CliWheel

    Write-Host ''
    Write-Host "Published bio_tools_app $cliVersion for Windows." -ForegroundColor Green
    Write-Host '    https://pypi.org/project/bio-tools-app'
    Write-Host ''
    Write-Host 'Run ./publish_cli_linux.sh on Linux, from this same commit, for the Linux wheel.' -ForegroundColor DarkGray
    exit 0
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
            $env:UV_PUBLISH_TOKEN = Read-PypiToken -Project 'athanor-bio-tools' -Variable 'UV_PUBLISH_TOKEN'
        }
    }
    if (-not $SkipCli) {
        Resolve-CliToken
    }
}

$currentVersion = Get-ManifestVersion -Path $rootManifest
$newVersion = if ($Version) { $Version } else { Step-Version -Current $currentVersion -Component $Bump }

Write-Host ''
Write-Host "bio_tools           $currentVersion -> $newVersion  (crates.io)" -ForegroundColor Green
Write-Host "athanor_bio_tools   $currentVersion -> $newVersion  (PyPI)" -ForegroundColor Green
Write-Host "bio_tools_app       $currentVersion -> $newVersion  (PyPI, Windows wheel)" -ForegroundColor Green
if ($SkipRust) { Write-Host 'Skipping crates.io.' -ForegroundColor Yellow }
if ($SkipPython) { Write-Host 'Skipping athanor_bio_tools.' -ForegroundColor Yellow }
if ($SkipCli) { Write-Host 'Skipping bio_tools_app.' -ForegroundColor Yellow }
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
# python_cli needs no edit: it builds the root crate, so it inherits the version set above.

# Neither wheel can read a file outside its own directory, so each gets a copy of the originals.
Write-Step 'Copying README.md and LICENSE into the wheel directories'
Copy-Item -LiteralPath $readme -Destination $pypiReadme -Force
Copy-Item -LiteralPath $readme -Destination $cliPypiReadme -Force
Copy-Item -LiteralPath $license -Destination $cliPypiLicense -Force

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

if (-not $SkipCli) {
    Build-CliWheel
}

if ($DryRun) {
    Write-Host ''
    Write-Host "Dry run complete. Version is set to $newVersion and artifacts are in python/dist and python_cli/dist." -ForegroundColor Green
    Write-Host 'Revert with: git checkout -- Cargo.toml Cargo.lock python/ python_cli/' -ForegroundColor DarkGray
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
    Invoke-Native -Description 'Publishing athanor_bio_tools to PyPI' -Program 'uv' -Arguments @('publish') `
        -WorkingDirectory $pythonDir
}

if (-not $SkipCli) {
    Publish-CliWheel
}

Write-Host ''
Write-Host "Published $newVersion." -ForegroundColor Green
if (-not $SkipRust) { Write-Host '    https://crates.io/crates/bio_tools' }
if (-not $SkipPython) { Write-Host '    https://pypi.org/project/athanor-bio-tools' }
if (-not $SkipCli) { Write-Host '    https://pypi.org/project/bio-tools-app  (Windows wheel)' }
Write-Host ''
Write-Host 'Note: both wheels are built for this platform only. For athanor_bio_tools, other' -ForegroundColor DarkGray
Write-Host 'platforms install from the sdist and need a Rust toolchain. bio_tools_app has no sdist,' -ForegroundColor DarkGray
Write-Host 'so Linux users get nothing until you run this from a Linux checkout at this commit:' -ForegroundColor DarkGray
Write-Host '    ./publish_cli_linux.sh' -ForegroundColor DarkGray
