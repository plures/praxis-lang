#!/usr/bin/env bash
# check-no-bom.sh — Fail if any Cargo.toml carries a UTF-8 BOM (EF BB BF).
#
# Why: a UTF-8 BOM on crate Cargo.toml manifests breaks downstream Nix builds.
# Nix's `replace-workspace-values` step parses manifests with strict tomli,
# which rejects a leading BOM as `TOMLDecodeError: Invalid statement (line 1,
# col 1)`. That failure cascades through every consumer (pluresdb-px ->
# pares-radix / pares-agens -> praxisbot system). Keep manifests BOM-free.
set -euo pipefail

bad=0
while IFS= read -r -d '' f; do
  # Read first 3 bytes; compare to the UTF-8 BOM.
  if [ "$(head -c3 "$f" | od -An -tx1 | tr -d ' \n')" = "efbbbf" ]; then
    echo "::error file=$f::UTF-8 BOM found at start of $f (strip it; breaks Nix replace-workspace-values)"
    bad=1
  fi
done < <(find . -name Cargo.toml -not -path './target/*' -print0)

if [ "$bad" -ne 0 ]; then
  echo "BOM check FAILED — one or more Cargo.toml files start with a UTF-8 BOM."
  exit 1
fi
echo "BOM check passed — no Cargo.toml carries a UTF-8 BOM."
