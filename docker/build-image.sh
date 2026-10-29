#!/usr/bin/env bash
set -euo pipefail

readonly PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
case "$(uname -m)" in
    x86_64)
        readonly TARGET="x86_64-unknown-linux-musl"
        ;;
    aarch64|arm64)
        readonly TARGET="aarch64-unknown-linux-musl"
        ;;
    *)
        echo "Unsupported host architecture: $(uname -m)." >&2
        echo "Only x86_64 and aarch64 are supported." >&2
        exit 1
        ;;
esac
readonly BINARY_NAME="ddns4cf"
readonly BUILD_DIR="${PROJECT_ROOT}/target/${TARGET}/release"

usage() {
    cat <<'EOF'
Usage: docker/build-image.sh

Builds ddns4f:<Cargo version> from a host-built static musl binary.
Supports x86_64 and aarch64 host architectures.
EOF
}

[[ $# -eq 0 ]] || {
    echo "This script does not accept arguments." >&2
    usage >&2
    exit 2
}

command -v docker >/dev/null 2>&1 || {
    echo "Docker is required to build the image." >&2
    exit 1
}

command -v cargo >/dev/null 2>&1 || {
    echo "Cargo is required to build the static binary." >&2
    exit 1
}

command -v musl-gcc >/dev/null 2>&1 || {
    echo "musl-gcc is required for static linking." >&2
    echo "Install it on Ubuntu or Debian with: sudo apt-get install musl-tools" >&2
    exit 1
}

if command -v rustup >/dev/null 2>&1 && ! rustup target list --installed | grep -Fxq "$TARGET"; then
    echo "Rust target ${TARGET} is not installed." >&2
    echo "Install it with: rustup target add ${TARGET}" >&2
    exit 1
fi

cargo build --manifest-path "${PROJECT_ROOT}/Cargo.toml" --release --target "$TARGET"

binary_path="${BUILD_DIR}/${BINARY_NAME}"
[[ -x "$binary_path" ]] || {
    echo "Expected compiled binary was not found: ${binary_path}" >&2
    exit 1
}

cargo_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "${PROJECT_ROOT}/Cargo.toml" | head -n 1)"
[[ -n "$cargo_version" ]] || {
    echo "Cannot read the package version from Cargo.toml." >&2
    exit 1
}

image_context="$(mktemp -d)"
trap 'rm -rf "$image_context"' EXIT
cp "$binary_path" "${image_context}/${BINARY_NAME}"
cp "${PROJECT_ROOT}/docker/Dockerfile" "${image_context}/Dockerfile"

docker build \
    --tag "ddns4f:${cargo_version}" \
    --tag "ddns4f:latest" \
    "$image_context"

echo "Built images: ddns4f:${cargo_version}, ddns4f:latest"