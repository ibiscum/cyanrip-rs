#!/usr/bin/env bash
set -euo pipefail

C_BIN_DEFAULT="/home/ulf/data/cyanrip/build/src/cyanrip"
C_BIN="${CYANRIP_C_BIN:-$C_BIN_DEFAULT}"

echo "Running M7 CLI differential first slice"
echo "C reference binary: $C_BIN"

if [[ ! -x "$C_BIN" ]]; then
  echo "C binary is missing or not executable: $C_BIN"
  echo "Set CYANRIP_C_BIN to a valid path."
  exit 1
fi

cargo test --features "backend-libcdio-sys paranoia" matches_upstream_short_option_surface
CYANRIP_C_BIN="$C_BIN" cargo test --features "backend-libcdio-sys paranoia" --test differential_cli_vs_c -- --ignored

echo "M7 CLI differential first slice completed."
