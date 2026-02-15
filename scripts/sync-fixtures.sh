#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RUST_REPO_DIR="${RUST_REPO_DIR:-$(dirname "$SCRIPT_DIR")}"
TS_REPO_DIR="${TS_REPO_DIR:-$RUST_REPO_DIR/../../ts-c2s-api}"

SRC="$RUST_REPO_DIR/docs/parity/fixtures"
DST="$TS_REPO_DIR/tests/fixtures/parity"

if [ ! -d "$SRC" ]; then
  echo "ERROR: Source fixtures not found at $SRC" >&2
  exit 1
fi

mkdir -p "$DST"
cp -v "$SRC"/*.json "$DST/"
echo "Synced fixtures from $SRC → $DST"
