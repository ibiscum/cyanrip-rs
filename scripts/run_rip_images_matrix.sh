#!/usr/bin/env bash
set -euo pipefail

# Runs a recommended rip-images scenario matrix for upstream and/or rs modes.
# Produces a compact pass/skip/fail summary.
#
# Usage:
#   run_rip_images_matrix.sh [--mode upstream|rs|both] [--fixtures <dir>] \
#     [--upstream-bin <path>] [--rs-bin <path>]

usage() {
  cat <<'EOF'
Usage: run_rip_images_matrix.sh [--mode upstream|rs|both] [--fixtures <dir>] [--upstream-bin <path>] [--rs-bin <path>]

Options:
  --mode <mode>          Which compatibility mode(s) to run: upstream, rs, both (default: both)
  --fixtures <dir>       Fixtures root directory (default: tests/fixtures)
  --upstream-bin <path>  Upstream C cyanrip binary path (default: /home/ulf/data/cyanrip/build/src/cyanrip)
  --rs-bin <path>        cyanrip-rs binary path (default: ./target/debug/cyanrip-rs)
  -h, --help             Show this help message

Notes:
  - This wrapper calls scripts/run_rip_images_scenarios.sh for each scenario.
  - Result classification:
      PASS: exit code 0 and no SKIP marker in output
      SKIP: exit code 0 and output contains "SKIP (<mode> mode):"
      FAIL: non-zero exit code
EOF
}

MODE="both"
FIXTURES_DIR="tests/fixtures"
UPSTREAM_BIN="/home/ulf/data/cyanrip/build/src/cyanrip"
RS_BIN="./target/debug/cyanrip-rs"
RUNNER="./scripts/run_rip_images_scenarios.sh"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --mode)
      if [[ $# -lt 2 ]]; then
        echo "Missing value for --mode"
        usage
        exit 2
      fi
      MODE="$2"
      shift 2
      ;;
    --fixtures)
      if [[ $# -lt 2 ]]; then
        echo "Missing value for --fixtures"
        usage
        exit 2
      fi
      FIXTURES_DIR="$2"
      shift 2
      ;;
    --upstream-bin)
      if [[ $# -lt 2 ]]; then
        echo "Missing value for --upstream-bin"
        usage
        exit 2
      fi
      UPSTREAM_BIN="$2"
      shift 2
      ;;
    --rs-bin)
      if [[ $# -lt 2 ]]; then
        echo "Missing value for --rs-bin"
        usage
        exit 2
      fi
      RS_BIN="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      usage
      exit 2
      ;;
  esac
done

case "$MODE" in
  upstream|rs|both)
    ;;
  *)
    echo "Invalid --mode value: $MODE"
    usage
    exit 2
    ;;
esac

if [[ ! -x "$RUNNER" ]]; then
  echo "Missing executable runner: $RUNNER"
  echo "Create it first or run: chmod +x $RUNNER"
  exit 1
fi

if [[ ! -d "$FIXTURES_DIR" ]]; then
  echo "Fixtures dir does not exist: $FIXTURES_DIR"
  exit 1
fi

UPSTREAM_SCENARIOS=(
  info
  basic
  pregap
  mixed
  nrg
  filters
  art
  art_back
  log_coverart
  cue_only
  errors
  multi_output_scheme
  verify_log
)

RS_SCENARIOS=(
  info
  basic
  pregap
  mixed
  nrg
  filters
  log_coverart
  cue_only
  errors
  verify_log
)

run_mode_matrix() {
  local compat="$1"
  local binary="$2"
  shift 2
  local scenarios=("$@")

  if [[ ! -x "$binary" ]]; then
    echo "[$compat] binary missing or not executable: $binary"
    return 1
  fi

  local pass=0
  local skip=0
  local fail=0

  echo
  echo "== $compat mode =="
  printf "%-10s %-22s %-6s\n" "MODE" "SCENARIO" "RESULT"

  local sc
  for sc in "${scenarios[@]}"; do
    local output
    local ec

    set +e
    output="$($RUNNER --compat "$compat" "$binary" "$FIXTURES_DIR" "$sc" 2>&1)"
    ec=$?
    set -e

    if [[ "$ec" -ne 0 ]]; then
      fail=$((fail + 1))
      printf "%-10s %-22s %-6s\n" "$compat" "$sc" "FAIL"
      continue
    fi

    if grep -Fq "SKIP ($compat mode):" <<<"$output"; then
      skip=$((skip + 1))
      printf "%-10s %-22s %-6s\n" "$compat" "$sc" "SKIP"
    else
      pass=$((pass + 1))
      printf "%-10s %-22s %-6s\n" "$compat" "$sc" "PASS"
    fi
  done

  echo "Summary [$compat]: pass=$pass skip=$skip fail=$fail"

  MODE_PASS["$compat"]=$pass
  MODE_SKIP["$compat"]=$skip
  MODE_FAIL["$compat"]=$fail
}

declare -A MODE_PASS=()
declare -A MODE_SKIP=()
declare -A MODE_FAIL=()

if [[ "$MODE" == "upstream" || "$MODE" == "both" ]]; then
  run_mode_matrix "upstream" "$UPSTREAM_BIN" "${UPSTREAM_SCENARIOS[@]}"
fi

if [[ "$MODE" == "rs" || "$MODE" == "both" ]]; then
  run_mode_matrix "rs" "$RS_BIN" "${RS_SCENARIOS[@]}"
fi

total_pass=0
total_skip=0
total_fail=0

if [[ "$MODE" == "upstream" || "$MODE" == "both" ]]; then
  total_pass=$((total_pass + ${MODE_PASS[upstream]:-0}))
  total_skip=$((total_skip + ${MODE_SKIP[upstream]:-0}))
  total_fail=$((total_fail + ${MODE_FAIL[upstream]:-0}))
fi

if [[ "$MODE" == "rs" || "$MODE" == "both" ]]; then
  total_pass=$((total_pass + ${MODE_PASS[rs]:-0}))
  total_skip=$((total_skip + ${MODE_SKIP[rs]:-0}))
  total_fail=$((total_fail + ${MODE_FAIL[rs]:-0}))
fi

echo
echo "== Overall summary =="
echo "pass=$total_pass skip=$total_skip fail=$total_fail"

if [[ "$total_fail" -ne 0 ]]; then
  exit 1
fi
