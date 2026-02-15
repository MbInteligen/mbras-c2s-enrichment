#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RUST_REPO_DIR="${RUST_REPO_DIR:-$(dirname "$SCRIPT_DIR")}"
TS_REPO_DIR="${TS_REPO_DIR:-$RUST_REPO_DIR/../../ts-c2s-api}"

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

echo "=== Step 1: Drift guard ==="
"$SCRIPT_DIR/check-fixture-hash.sh"

echo "=== Step 2: Rust fixture output ==="
cd "$RUST_REPO_DIR"
cargo test --test scoring_fixtures -- --show-output 2>/dev/null \
  | grep '^PARITY_JSON:' | sed 's/^PARITY_JSON://' \
  | jq -S -c '.' | sort > "$TMPDIR/parity-rust.jsonl"
echo "  $(wc -l < "$TMPDIR/parity-rust.jsonl") cases from Rust"

echo "=== Step 3: TS fixture output ==="
cd "$TS_REPO_DIR"
bun test tests/*-fixtures.test.ts 2>/dev/null \
  | grep '^PARITY_JSON:' | sed 's/^PARITY_JSON://' \
  | jq -S -c '.' | sort > "$TMPDIR/parity-ts.jsonl"
echo "  $(wc -l < "$TMPDIR/parity-ts.jsonl") cases from TS"

echo "=== Step 4: Diff ==="
if diff "$TMPDIR/parity-rust.jsonl" "$TMPDIR/parity-ts.jsonl"; then
  echo "PARITY OK"
else
  echo "PARITY MISMATCH"
  exit 1
fi
