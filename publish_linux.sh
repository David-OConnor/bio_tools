#!/usr/bin/env bash
#
# Builds and publishes the Linux wheels for both PyPI packages:
#   athanor_bio_tools  the Python library (an extension module built from python/)
#   bio_tools_app      the CLI application (the prebuilt `bio_tools` executable, from python_cli/)
#
# Run this after `publish.ps1`, which publishes the rust crate and the Windows wheels.
#
# Both wheels contain compiled code, so they are per-platform and each has to be built on its own
# platform: this covers Linux, and publish.ps1 covers Windows. athanor_bio_tools also has an sdist,
# uploaded once by publish.ps1, which is what platforms with no wheel build from — but that needs a
# Rust toolchain on the installing machine, which is exactly what the wheel here avoids.
# Everything else about a release — the version bump, the git commit, crates.io, and the sdist —
# belongs to publish.ps1; run this afterwards, from a checkout at the commit publish.ps1 pushed.
#
# The version is whatever Cargo.toml already says; nothing here bumps, commits, or tags.
#
# Usage:
#   ./publish_linux.sh              # build both, confirm, publish
#   ./publish_linux.sh --dry-run    # build only; wheels are left in python/dist and python_cli/dist
#   ./publish_linux.sh --yes        # no confirmation prompt
#   ./publish_linux.sh --no-zig     # link against the host glibc instead of an older one
#   ./publish_linux.sh --lib-only   # just athanor_bio_tools
#   ./publish_linux.sh --cli-only   # just bio_tools_app
#
# The PyPI tokens are read from $UV_PUBLISH_TOKEN (athanor_bio_tools) and
# $BIO_TOOLS_APP_PYPI_TOKEN (bio_tools_app), or asked for if unset. They are separate variables
# because a token scoped to one project cannot upload to the other.

set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
lib_dir="$root/python"
cli_dir="$root/python_cli"

dry_run=0
assume_yes=0
use_zig=1
do_lib=1
do_cli=1

for arg in "$@"; do
    case "$arg" in
        --dry-run) dry_run=1 ;;
        --yes | -y) assume_yes=1 ;;
        --no-zig) use_zig=0 ;;
        --lib-only) do_cli=0 ;;
        --cli-only) do_lib=0 ;;
        -h | --help)
            # The header comment above, minus the shebang and the leading `# `.
            awk 'NR > 2 && /^#/ { sub(/^# ?/, ""); print; next } NR > 2 { exit }' "${BASH_SOURCE[0]}"
            exit 0
            ;;
        *)
            echo "Unknown argument: $arg" >&2
            exit 2
            ;;
    esac
done

[ "$do_lib" -eq 1 ] || [ "$do_cli" -eq 1 ] || {
    echo '--lib-only and --cli-only cannot be combined; there would be nothing left to publish.' >&2
    exit 2
}

step() { printf '\n\033[36m==> %s\033[0m\n' "$1"; }
note() { printf '\033[90m    %s\033[0m\n' "$1"; }

for tool in cargo uv; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "$tool is not on PATH; it is required to publish." >&2
        exit 1
    }
done

# maturin is run through uvx, so there is nothing to install or keep up to date by hand. The
# version bound matches the build requirement in both packages' pyproject.toml.
maturin=(uvx --from 'maturin>=1.9,<2.0' maturin)
maturin_zig=(uvx --from 'maturin[zig]>=1.9,<2.0' maturin)

version="$(sed -n 's/^version *= *"\([0-9][^"]*\)".*/\1/p' "$root/Cargo.toml" | head -n 1)"
[ -n "$version" ] || {
    echo "Could not find a package version in $root/Cargo.toml." >&2
    exit 1
}

# Neither wheel can read a file outside its own directory, so each gets a copy of the originals.
# maturin reads the long description from these; keep them in step with the source of truth,
# exactly as publish.ps1 does for its own run.
cp -f "$root/README.md" "$lib_dir/PYPI_README.md"
cp -f "$root/LICENSE" "$lib_dir/PYPI_LICENSE"
cp -f "$root/README.md" "$cli_dir/PYPI_README.md"
cp -f "$root/LICENSE" "$cli_dir/PYPI_LICENSE"

platform="$(uname -s)-$(uname -m)"
printf '\n'
[ "$do_lib" -eq 1 ] && printf '\033[32mathanor_bio_tools   %s   (PyPI, %s wheel)\033[0m\n' "$version" "$platform"
[ "$do_cli" -eq 1 ] && printf '\033[32mbio_tools_app       %s   (PyPI, %s wheel)\033[0m\n' "$version" "$platform"
note 'Only wheels are published; no version bump, no commit, no sdist, no crates.io.'
[ "$dry_run" -eq 1 ] && echo 'Dry run: nothing will be published.'

# Nothing here is Linux-specific except the zig step, so macOS wheels work too. On Windows,
# `./publish.ps1` is the equivalent and is what the README points at.
case "$(uname -s)" in
    Linux) ;;
    *) note "Not Linux: this will build $(uname -s) wheels, not manylinux ones." ;;
esac

# Builds one wheel, into <dir>/dist, and leaves its path in $built_wheel. A global rather than a
# command substitution, so maturin's progress output still reaches the terminal as it builds.
#
# --zig links against an older glibc than the host's, so the manylinux tag covers distributions
# older than this machine. Without it the wheel is tagged for the host's glibc and refuses to
# install on anything older. It is a build-time nicety, not a requirement, so a failure here falls
# back rather than aborting the release.
#
# Wheel only, and no sdist: bio_tools_app has none by design, and athanor_bio_tools' sdist is
# uploaded once by publish.ps1 — PyPI rejects a second upload of the same filename.
build_wheel() {
    local dir="$1" label="$2" dist="$1/dist" built=0

    # Stale artifacts here would be picked up below and re-uploaded, then rejected by PyPI.
    rm -rf "$dist"

    if [ "$use_zig" -eq 1 ] && [ "$(uname -s)" = "Linux" ]; then
        step "Building the $label wheel (manylinux2014, via zig)"
        # Run from the package directory so maturin picks up its pyproject.toml.
        if (cd "$dir" && "${maturin_zig[@]}" build --release --zig \
            --compatibility manylinux2014 --out dist); then
            built=1
        else
            note 'zig build failed; falling back to a host-glibc build.'
            rm -rf "$dist"
        fi
    fi

    if [ "$built" -eq 0 ]; then
        step "Building the $label wheel"
        (cd "$dir" && "${maturin[@]}" build --release --out dist)
    fi

    built_wheel="$(ls "$dist"/*.whl 2>/dev/null | head -n 1)"
    [ -n "$built_wheel" ] || {
        echo "No wheel was produced in $dist." >&2
        exit 1
    }
    note "$(basename "$built_wheel")"
}

# Asks for a PyPI API token, if the named variable is not already set. Credentials are collected
# before anything is published, so a missing token cannot strand us with one wheel uploaded and
# the other not.
resolve_token() {
    local var="$1" project="$2"
    [ -n "${!var:-}" ] && return 0

    step "PyPI login needed for $project"
    note 'Create an API token at https://pypi.org/manage/account/token/'
    note "(scope it to $project, or \"Entire account\" for the first upload)"
    # -s so the token is not echoed into the terminal's scrollback.
    local token
    read -rs -p '    Paste the token (starts with pypi-): ' token
    echo
    case "$token" in
        pypi-*) ;;
        *)
            echo 'That does not look like a PyPI API token; it should start with "pypi-".' >&2
            exit 1
            ;;
    esac
    printf -v "$var" '%s' "$token"
    note "Export $var in your shell profile to skip this next time."
}

# Uploads one already-built wheel. The token goes through the environment rather than `--token`,
# so it does not show up in this machine's process list while the upload runs.
publish_wheel() {
    local wheel="$1" label="$2" token="$3"
    step "Publishing $label to PyPI"
    UV_PUBLISH_TOKEN="$token" uv publish "$wheel"
}

# --- Build everything before publishing anything ---------------------------------------
#
# A broken build should never leave one package published and the other not.

lib_wheel=''
cli_wheel=''
built_wheel=''
if [ "$do_lib" -eq 1 ]; then
    build_wheel "$lib_dir" 'athanor_bio_tools'
    lib_wheel="$built_wheel"
fi
if [ "$do_cli" -eq 1 ]; then
    build_wheel "$cli_dir" 'bio_tools_app'
    cli_wheel="$built_wheel"
fi

if [ "$dry_run" -eq 1 ]; then
    printf '\n\033[32mDry run complete. The wheels are in python/dist and python_cli/dist.\033[0m\n'
    exit 0
fi

# --- Credentials and confirmation ------------------------------------------------------

[ "$do_lib" -eq 1 ] && resolve_token UV_PUBLISH_TOKEN 'athanor-bio-tools'
[ "$do_cli" -eq 1 ] && resolve_token BIO_TOOLS_APP_PYPI_TOKEN 'bio-tools-app'

if [ "$assume_yes" -eq 0 ]; then
    read -r -p "Publish $version to PyPI? Releases cannot be undone [y/N] " answer
    case "$answer" in
        y | Y | yes) ;;
        *)
            echo 'Aborted; nothing was published.'
            exit 1
            ;;
    esac
fi

# --- Publish ---------------------------------------------------------------------------

[ "$do_lib" -eq 1 ] && publish_wheel "$lib_wheel" 'athanor_bio_tools' "$UV_PUBLISH_TOKEN"
[ "$do_cli" -eq 1 ] && publish_wheel "$cli_wheel" 'bio_tools_app' "$BIO_TOOLS_APP_PYPI_TOKEN"

printf '\n\033[32mPublished %s for %s.\033[0m\n' "$version" "$platform"
[ "$do_lib" -eq 1 ] && echo '    https://pypi.org/project/athanor-bio-tools'
[ "$do_cli" -eq 1 ] && echo '    https://pypi.org/project/bio-tools-app'
exit 0
