#!/bin/sh

set -eu

usage() {
    cat <<'EOF'
Usage: scripts/ci-checks.sh [--fuse | --fuse-only]

  no option    Run formatting, tests, and Clippy.
  --fuse       Run the standard checks and the FUSE feature build.
  --fuse-only  Run only the FUSE feature build.
EOF
}

run_standard_checks=true
run_fuse_check=false

case "${1:-}" in
    "")
        ;;
    --fuse)
        run_fuse_check=true
        ;;
    --fuse-only)
        run_standard_checks=false
        run_fuse_check=true
        ;;
    -h|--help)
        usage
        exit 0
        ;;
    *)
        usage >&2
        exit 2
        ;;
esac

repository_root=$(git rev-parse --show-toplevel)
cd "$repository_root"

if [ "$run_standard_checks" = true ]; then
    echo "Checking Rust formatting"
    cargo fmt --all --check

    echo "Running Rust tests"
    cargo test --locked --all-targets

    echo "Running Clippy"
    cargo clippy --locked --all-targets --no-default-features -- -D warnings
fi

if [ "$run_fuse_check" = true ]; then
    echo "Checking the FUSE feature build"
    cargo check --locked --features macfuse
fi

