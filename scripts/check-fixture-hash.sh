#!/bin/bash
set -euo pipefail

# Portable SHA-256: prefer sha256sum, fallback to shasum -a 256 (macOS)
if command -v sha256sum &>/dev/null; then
  HASH_CMD="sha256sum"
elif command -v shasum &>/dev/null; then
  HASH_CMD="shasum -a 256"
else
  echo "ERROR: No sha256sum or shasum found" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RUST_REPO_DIR="${RUST_REPO_DIR:-$(dirname "$SCRIPT_DIR")}"
TS_REPO_DIR="${TS_REPO_DIR:-$RUST_REPO_DIR/../../ts-c2s-api}"

RUST_FIXTURES="$RUST_REPO_DIR/docs/parity/fixtures"
TS_FIXTURES="$TS_REPO_DIR/tests/fixtures/parity"

if [ ! -d "$RUST_FIXTURES" ]; then
  echo "ERROR: Rust fixtures not found at $RUST_FIXTURES" >&2
  exit 1
fi

if [ ! -d "$TS_FIXTURES" ]; then
  echo "ERROR: TS fixtures not found at $TS_FIXTURES — run scripts/sync-fixtures.sh first" >&2
  exit 1
fi

DRIFT=0
for f in "$RUST_FIXTURES"/*.json; do
  fname=$(basename "$f")
  ts_file="$TS_FIXTURES/$fname"
  if [ ! -f "$ts_file" ]; then
    echo "MISSING: $fname not found in TS fixtures"
    DRIFT=1
    continue
  fi
  rust_hash=$($HASH_CMD "$f" | awk '{print $1}')
  ts_hash=$($HASH_CMD "$ts_file" | awk '{print $1}')
  if [ "$rust_hash" != "$ts_hash" ]; then
    echo "DRIFT: $fname — Rust=$rust_hash TS=$ts_hash"
    DRIFT=1
  fi
done

if [ "$DRIFT" -eq 1 ]; then
  echo "FIXTURE DRIFT DETECTED — run scripts/sync-fixtures.sh"
  exit 1
fi

echo "FIXTURES OK — no drift detected"
