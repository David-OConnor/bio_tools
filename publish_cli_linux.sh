#!/usr/bin/env bash
#
# Builds and publishes the Linux `bio_tools_app` wheel.
#
# The wheel contains the prebuilt `bio_tools` executable and no Python code, so
# `pip install bio_tools_app` puts `bio_tools` on PATH without a Rust toolchain. Wheels are
# per-platform, so each one has to be built on its own platform: this covers Linux, and
# `./publish.ps1 -CliOnly` covers Windows. Everything else about a release — the version bump, the
# git commit, crates.io, and the athanor_bio_tools library wheel — belongs to publish.ps1; run
# this afterwards, from a checkout at the commit publish.ps1 pushed.
#
# The version is whatever Cargo.toml already says; nothing here bumps, commits, or tags.
#
# Usage:
#   ./publish_cli_linux.sh              # build, confirm, publish
#   ./publish_cli_linux.sh --dry-run    # build only; the wheel is left in python_cli/dist
#   ./publish_cli_linux.sh --yes        # no confirmation prompt
#   ./publish_cli_linux.sh --no-zig     # link against the host glibc instead of an older one
#
# The PyPI token is read from $BIO_TOOLS_APP_PYPI_TOKEN, or asked for if unset.

set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cli_dir="$root/python_cli"
dist_dir="$cli_dir/dist"

dry_run=0
assume_yes=0
use_zig=1

for arg in "$@"; do
    case "$arg" in
        --dry-run) dry_run=1 ;;
        --yes | -y) assume_yes=1 ;;
        --no-zig) use_zig=0 ;;
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

step() { printf '\n\033[36m==> %s\033[0m\n' "$1"; }
note() { printf '\033[90m    %s\033[0m\n' "$1"; }

for tool in cargo uv; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "$tool is not on PATH; it is required to publish." >&2
        exit 1
    }
done

# maturin is run through uvx, so there is nothing to install or keep up to date by hand. The
# version bound matches python_cli/pyproject.toml's build requirement.
maturin=(uvx --from 'maturin>=1.9,<2.0' maturin)
maturin_zig=(uvx --from 'maturin[zig]>=1.9,<2.0' maturin)

version="$(sed -n 's/^version *= *"\([0-9][^"]*\)".*/\1/p' "$root/Cargo.toml" | head -n 1)"
[ -n "$version" ] || {
    echo "Could not find a package version in $root/Cargo.toml." >&2
    exit 1
}

# maturin reads the wheel's long description from this copy; keep it in step with the source of
# truth, exactly as publish.ps1 does for its own packages.
cp -f "$root/README.md" "$cli_dir/PYPI_README.md"
cp -f "$root/LICENSE" "$cli_dir/PYPI_LICENSE"

printf '\n\033[32mbio_tools_app   %s   (PyPI, %s wheel)\033[0m\n' "$version" "$(uname -s)-$(uname -m)"
note 'Only this wheel is published; no version bump, no commit, no other package.'
[ "$dry_run" -eq 1 ] && echo "Dry run: nothing will be published."

# Nothing here is Linux-specific except the zig step, so a macOS wheel works too. On Windows,
# `./publish.ps1 -CliOnly` is the equivalent and is what the README points at.
case "$(uname -s)" in
    Linux) ;;
    *) note "Not Linux: this will build a $(uname -s) wheel, not a manylinux one." ;;
esac

# Stale artifacts here would be re-uploaded by `uv publish` and rejected by PyPI.
rm -rf "$dist_dir"

# --zig links against an older glibc than the host's, so the manylinux tag covers distributions
# older than this machine. Without it the wheel is tagged for the host's glibc and refuses to
# install on anything older. It is a build-time nicety, not a requirement, so a failure here falls
# back rather than aborting the release.
built=0
if [ "$use_zig" -eq 1 ] && [ "$(uname -s)" = "Linux" ]; then
    step "Building the wheel (manylinux2014, via zig)"
    # Run from python_cli so maturin picks up its pyproject.toml, which names the distribution and
    # points at the crate in the parent directory.
    if (cd "$cli_dir" && "${maturin_zig[@]}" build --release --zig \
        --compatibility manylinux2014 --out dist); then
        built=1
    else
        note 'zig build failed; falling back to a host-glibc build.'
        rm -rf "$dist_dir"
    fi
fi

if [ "$built" -eq 0 ]; then
    step 'Building the wheel'
    (cd "$cli_dir" && "${maturin[@]}" build --release --out dist)
fi

wheel="$(ls "$dist_dir"/*.whl 2>/dev/null | head -n 1)"
[ -n "$wheel" ] || {
    echo "No wheel was produced in $dist_dir." >&2
    exit 1
}
note "$(basename "$wheel")"

if [ "$dry_run" -eq 1 ]; then
    printf '\n\033[32mDry run complete. The wheel is in python_cli/dist.\033[0m\n'
    exit 0
fi

if [ -z "${BIO_TOOLS_APP_PYPI_TOKEN:-}" ]; then
    step 'PyPI login needed'
    note 'Create an API token at https://pypi.org/manage/account/token/'
    note '(scope it to bio-tools-app, or "Entire account" for the first upload)'
    # -s so the token is not echoed into the terminal's scrollback.
    read -rs -p '    Paste the token (starts with pypi-): ' BIO_TOOLS_APP_PYPI_TOKEN
    echo
    case "$BIO_TOOLS_APP_PYPI_TOKEN" in
        pypi-*) ;;
        *)
            echo 'That does not look like a PyPI API token; it should start with "pypi-".' >&2
            exit 1
            ;;
    esac
    note 'Export BIO_TOOLS_APP_PYPI_TOKEN in your shell profile to skip this next time.'
fi

if [ "$assume_yes" -eq 0 ]; then
    read -r -p "Publish bio_tools_app $version? Releases cannot be undone [y/N] " answer
    case "$answer" in
        y | Y | yes) ;;
        *)
            echo 'Aborted; nothing was published.'
            exit 1
            ;;
    esac
fi

step 'Publishing to PyPI'
# Through the environment rather than `--token`, so the token does not show up in this machine's
# process list while the upload runs.
UV_PUBLISH_TOKEN="$BIO_TOOLS_APP_PYPI_TOKEN" uv publish "$dist_dir"/*.whl

printf '\n\033[32mPublished bio_tools_app %s for %s.\033[0m\n' "$version" "$(uname -s)-$(uname -m)"
echo '    https://pypi.org/project/bio-tools-app'
