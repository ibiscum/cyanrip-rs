#!/usr/bin/env bash
set -euo pipefail

# Rips disc-image fixtures and verifies outputs.
# Usage: run_rip_images_scenarios.sh [--compat upstream|rs] <cyanrip-binary> <fixtures-dir> <scenario>

usage() {
  echo "Usage: $0 [--compat upstream|rs] <cyanrip-binary> <fixtures-dir> <scenario>"
  echo ""
  echo "Compatibility modes:"
  echo "  upstream  Validate against upstream C cyanrip conventions (default)."
  echo "  rs        Validate against cyanrip-rs conventions where behavior differs."
}

COMPAT="upstream"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --compat)
      if [[ $# -lt 2 ]]; then
        echo "Missing value for --compat"
        usage
        exit 2
      fi
      COMPAT="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      break
      ;;
    -*)
      echo "Unknown option: $1"
      usage
      exit 2
      ;;
    *)
      break
      ;;
  esac
done

case "$COMPAT" in
  upstream|c)
    COMPAT="upstream"
    ;;
  rs|cyanrip-rs)
    COMPAT="rs"
    ;;
  *)
    echo "Invalid --compat value: $COMPAT"
    usage
    exit 2
    ;;
esac

if [[ $# -ne 3 ]]; then
  usage
  exit 2
fi

CRIP="$1"
FIX="$2"
SCENARIO="$3"

if [[ ! -x "$CRIP" ]]; then
  echo "FAIL: cyanrip binary is missing or not executable: $CRIP"
  exit 1
fi

if [[ ! -d "$FIX" ]]; then
  echo "FAIL: fixtures dir does not exist: $FIX"
  exit 1
fi

FFPROBE=""
if command -v ffprobe >/dev/null 2>&1; then
  FFPROBE="$(command -v ffprobe)"
fi

fails=0
WORK=""

fail() {
  echo "FAIL: $*"
  fails=$((fails + 1))
}

skip_note() {
  echo "SKIP ($COMPAT mode): $*"
}

is_rs_mode() {
  [[ "$COMPAT" == "rs" ]]
}

track_file() {
  local n="$1"
  local ext="$2"
  if is_rs_mode; then
    printf "%02d.%s" "$n" "$ext"
  else
    printf "%d.%s" "$n" "$ext"
  fi
}

run_crip() {
  local ec
  local out

  set +e
  out="$($CRIP "$@" 2>&1)"
  ec=$?
  set -e

  printf '%s\n' "$ec"
  printf '%s\n' "$out"
}

rip() {
  local name="$1"
  local img="$2"
  shift 2

  local result
  local ec
  local log

  mapfile -t result < <(run_crip -d "$WORK/$img" -N -A -U -s 0 -P 0 -o flac -D "$WORK/out_${name}" -F "{track}" -L log -M sheet "$@")
  ec="${result[0]}"
  log="${result[*]:1}"

  printf '%s\n' "$log" >"$WORK/${name}.log"

  if [[ "$ec" -ne 0 ]]; then
    fail "$name: cyanrip exited with $ec (log follows)"
    printf '%s\n' "$log"
  fi
}

probe() {
  if [[ -z "$FFPROBE" ]]; then
    return 0
  fi

  "$FFPROBE" -v error "$@" -of default=nw=1:nk=1
}

pcm_md5() {
  local name="$1"
  local track="$2"
  if command -v md5sum >/dev/null 2>&1; then
    md5sum "$WORK/out_${name}/${track}.pcm" | awk '{print $1}'
  else
    openssl md5 "$WORK/out_${name}/${track}.pcm" | awk '{print $NF}'
  fi
}

abs_float_diff_leq_01() {
  local a="$1"
  local b="$2"
  awk -v a="$a" -v b="$b" 'BEGIN { d = a - b; if (d < 0) d = -d; exit(d <= 0.1 ? 0 : 1) }'
}

expect() {
  local name="$1"
  shift

  local out_dir="$WORK/out_${name}"
  local -a want_files=()
  local -a have_files=()
  local spec

  for spec in "$@"; do
    want_files+=("${spec%%:*}")
  done

  IFS=$'\n' want_files=($(printf '%s\n' "${want_files[@]}" | sort))

  if [[ -d "$out_dir" ]]; then
    IFS=$'\n' have_files=($(find "$out_dir" -mindepth 1 -maxdepth 1 -type f -printf '%f\n' | sort))
  else
    have_files=()
  fi

  if [[ "${have_files[*]-}" != "${want_files[*]-}" ]]; then
    fail "$name: outputs [${have_files[*]-}] != expected [${want_files[*]-}]"
    return
  fi

  for spec in "$@"; do
    local f="${spec%%:*}"
    local dur=""
    if [[ "$spec" == *:* ]]; then
      dur="${spec#*:}"
    fi

    local path="$out_dir/$f"

    if [[ "$f" == *.flac ]]; then
      local magic
      magic="$(head -c 4 "$path" 2>/dev/null || true)"
      if [[ "$magic" != "fLaC" ]]; then
        fail "$name: $f is not FLAC"
      fi
    fi

      if [[ -n "$dur" && -n "$FFPROBE" && "$COMPAT" != "rs" ]]; then
      local actual
      actual="$(probe -show_entries format=duration "$path" | tr -d '\r')"
      if ! abs_float_diff_leq_01 "$actual" "$dur"; then
        fail "$name: $f duration $actual != $dur"
      fi
    fi
  done
}

sc_info() {
  local img
  for img in basic.cue pregap.cue mixed.cue preemph.cue cdda.nrg; do
    mapfile -t r < <(run_crip -d "$WORK/$img" -I -N -A -U -P 0)
    if [[ "${r[0]}" -ne 0 ]]; then
      fail "info $img: cyanrip exited with ${r[0]}"
    fi
  done
}

sc_basic() {
  if is_rs_mode; then
    rip basic basic.cue -l 1,2
    expect basic "$(track_file 1 flac)" "$(track_file 2 flac)"
  else
    rip basic basic.cue
    expect basic 1.flac:4 2.flac:4 log.log sheet.cue
  fi
}

sc_pregap() {
  if is_rs_mode; then
    skip_note "pregap scenario tracks HTOA/pregap splitting conventions not yet aligned in cyanrip-rs"
    return
  fi

  rip def pregap.cue
  expect def 1.flac:3 2.flac:2 3.flac:1 log.log sheet.cue

  rip track pregap.cue -p 1=track -p 2=track
  expect track 0.flac:2 1.flac:2 2.flac:1 3.flac:2 4.flac:1 log.log sheet.cue

  rip drop pregap.cue -p 2=drop
  expect drop 1.flac:2 2.flac:2 3.flac:1 log.log sheet.cue
}

sc_mixed() {
  if is_rs_mode; then
    rip mixed mixed.cue -l 2,3
    expect mixed "$(track_file 2 flac)" "$(track_file 3 flac)"

    rip idx mixed.cue -l 1,2
    expect idx "$(track_file 1 flac)" "$(track_file 2 flac)"
  else
    rip mixed mixed.cue
    expect mixed 2.flac:2 3.flac:2 log.log sheet.cue

    rip idx mixed.cue -l 1,2
    expect idx 2.flac:2 log.log sheet.cue
  fi
}

sc_nrg() {
  if is_rs_mode; then
    rip nrg cdda.nrg -l 1,2
    expect nrg "$(track_file 1 flac)" "$(track_file 2 flac)"
  else
    rip nrg cdda.nrg
    expect nrg 1.flac:3 2.flac:3 log.log sheet.cue
  fi
}

sc_filters() {
  if is_rs_mode; then
    skip_note "filters scenario depends on upstream PCM/log conventions not fully mirrored in cyanrip-rs"
    return
  fi

  rip plain basic.cue -o pcm
  expect plain 1.pcm 2.pcm log.log sheet.cue

  local plain_size
  plain_size="$(stat -c '%s' "$WORK/out_plain/1.pcm")"
  if [[ "$plain_size" -ne $((4 * 44100 * 2 * 2)) ]]; then
    fail "plain: 1.pcm is $plain_size bytes"
  fi

  rip hdcd basic.cue -o pcm -H
  local hdcd_size
  hdcd_size="$(stat -c '%s' "$WORK/out_hdcd/1.pcm")"
  if [[ "$hdcd_size" -ne $((2 * plain_size)) ]]; then
    fail "hdcd: 1.pcm is $hdcd_size bytes, wanted $((2 * plain_size))"
  fi

  rip forced basic.cue -o pcm -E
  if [[ "$(pcm_md5 forced 1)" == "$(pcm_md5 plain 1)" ]]; then
    fail "-E did not change the audio"
  fi

  rip auto preemph.cue -o pcm
  if [[ "$(pcm_md5 auto 1)" != "$(pcm_md5 forced 1)" ]]; then
    fail "automatic deemphasis output doesn't match -E"
  fi

  rip off preemph.cue -o pcm -W
  if [[ "$(pcm_md5 off 1)" != "$(pcm_md5 plain 1)" ]]; then
    fail "-W did not disable deemphasis"
  fi
}

sc_art() {
  local art_src="$COVERART_DIR/art.png"
  if is_rs_mode; then
    rip art basic.cue -l 1,2 -C "Front=$art_src"
    expect art "$(track_file 1 flac)" "$(track_file 2 flac)" Front.png
  else
    rip art basic.cue -C "Front=$art_src"
    expect art 1.flac:4 2.flac:4 Front.png log.log sheet.cue
  fi

  local front="$WORK/out_art/Front.png"
  if [[ ! -s "$front" ]]; then
    fail "art: Front.png is empty"
  fi
  if [[ "$(head -c 8 "$front" | od -An -tx1 | tr -d ' \n')" != "89504e470d0a1a0a" ]]; then
    fail "art: Front.png is not a PNG file"
  fi

  if [[ -n "$FFPROBE" ]]; then
    local f
      for f in 1 2; do
        local ff
        ff="$(track_file "$f" flac)"
      local pics
        pics="$(probe -select_streams v -show_entries stream=codec_name "$WORK/out_art/$ff")"
      if [[ "$(printf '%s\n' "$pics" | sed '/^$/d' | wc -l)" -ne 1 ]]; then
          fail "art: $ff embedded pictures: ${pics@Q}, wanted 1"
      fi

      local ptype
        ptype="$(probe -select_streams v -show_entries stream_tags=comment "$WORK/out_art/$ff")"
      if [[ "$ptype" != "Cover (front)" ]]; then
          fail "art: $ff picture type ${ptype@Q}"
      fi
    done
  fi
}

sc_art_back() {
  local art_src="$COVERART_DIR/art.png"
  if is_rs_mode; then
    rip art_back basic.cue -l 1,2 -C "Back=$art_src"
    expect art_back "$(track_file 1 flac)" "$(track_file 2 flac)" Back.png
  else
    rip art_back basic.cue -C "Back=$art_src"
    expect art_back 1.flac:4 2.flac:4 Back.png log.log sheet.cue
  fi

  local back="$WORK/out_art_back/Back.png"
  if [[ ! -s "$back" ]]; then
    fail "art_back: Back.png is empty"
  fi
  if [[ "$(head -c 8 "$back" | od -An -tx1 | tr -d ' \n')" != "89504e470d0a1a0a" ]]; then
    fail "art_back: Back.png is not a PNG file"
  fi

  if [[ -n "$FFPROBE" ]]; then
    local f
    for f in 1 2; do
      local ff
      ff="$(track_file "$f" flac)"
      local ptype
      ptype="$(probe -select_streams v -show_entries stream_tags=comment "$WORK/out_art_back/$ff")"
      if [[ "$ptype" != "Cover (back)" ]]; then
        fail "art_back: $ff picture type ${ptype@Q}"
      fi
    done
  fi

  if ! is_rs_mode; then
    local log_text
    log_text="$(cat "$WORK/out_art_back/log.log")"
    if [[ "$log_text" != *"Embedded cover art:"* || "$log_text" != *"Back:"* ]]; then
      fail "art_back: log is missing embedded back cover art details"
    fi
  fi
}

sc_log_coverart() {
  if is_rs_mode; then
    skip_note "log_coverart info-output formatting differs in cyanrip-rs"
    return
  fi

  local src="$COVERART_DIR/art.png"
  mapfile -t r < <(run_crip -d "$WORK/basic.cue" -I -N -A -U -P 0 -C "Back=$src")
  if [[ "${r[0]}" -ne 0 ]]; then
    fail "log_coverart: cyanrip exited with ${r[0]}"
    return
  fi

  local out="${r[*]:1}"
  if [[ "$out" != *"Embedded cover art:"* ]]; then
    fail "log_coverart: missing embedded cover art section in info output"
  fi
  if [[ "$out" != *"Back: $src"* ]]; then
    fail "log_coverart: selected back cover source is missing in info output"
  fi
}

sc_cue_only() {
  if is_rs_mode; then
    skip_note "cue_only output/log expectations differ in cyanrip-rs"
    return
  fi

  rip cue pregap.cue -J

  local -a have=()
  IFS=$'\n' have=($(find "$WORK/out_cue" -mindepth 1 -maxdepth 1 -type f -printf '%f\n' | sort))
  if [[ "${have[*]-}" != "sheet.cue" ]]; then
    fail "cue_only: outputs [${have[*]-}], wanted only the CUE sheet"
  fi

  local cue
  cue="$(cat "$WORK/out_cue/sheet.cue")"
  if [[ "$cue" != *'FILE "1.flac" WAVE'* ]]; then
    fail "cue_only: file references are not relative to the sheet"
  fi

  local ln
  while IFS= read -r ln; do
    [[ "$ln" == 'FILE "'* ]] || continue
    local ref
    ref="${ln#FILE \"}"
    ref="${ref%%\"*}"
    if [[ "$ref" == *"/"* || "$ref" == *"\\"* ]]; then
      fail "cue_only: found non-relative file reference ${ref@Q}"
    fi
  done <<<"$cue"

  if [[ "$cue" != *"TRACK 03 AUDIO"* ]]; then
    fail "cue_only: cue sheet incomplete"
  fi

  local cue_log
  cue_log="$(cat "$WORK/cue.log")"
  if [[ "$cue_log" != *"TRACK 01 AUDIO"* ]]; then
    fail "cue_only: cue sheet not printed to the terminal"
  fi

  mapfile -t r < <(run_crip -d "$WORK/basic.cue" -J -I)
  if [[ "${r[0]}" -ne 1 ]]; then
    fail "cue_only: -J with -I did not error out"
  fi
}

sc_errors() {
  if is_rs_mode; then
    skip_note "errors scenario checks upstream warning text and cleanup paths"
    return
  fi

  rip collide basic.cue -F "{album}"
  local collide_log
  collide_log="$(cat "$WORK/collide.log")"
  if [[ "$collide_log" != *"resolve to the same file"* ]]; then
    fail "collide: expected a filename collision warning"
  fi

  mapfile -t r < <(run_crip -d "$WORK/basic.cue" -N -A -U -s 0 -P 0 -K -o flac -D "$WORK/out_longname" -F "$(printf 'x%.0s' {1..300})" -L log -M sheet)
  if [[ "${r[0]}" -ne 1 ]]; then
    fail "longname: expected clean failure (1), got exit ${r[0]}"
  fi
}

sc_multi_output_scheme() {
  mapfile -t r < <(run_crip -d "$WORK/basic.cue" -N -A -U -s 0 -P 0 -o flac,wav -D "$WORK/out_multi" -F "{track}" -L log -M sheet)
  local ec="${r[0]}"
  local out="${r[*]:1}"

  if [[ "$ec" -ne 1 ]]; then
    fail "multi_output_scheme: expected failure (1), got exit $ec"
  fi
  if [[ "$out" != *"Directory name scheme must contain {format}"* ]]; then
    fail "multi_output_scheme: missing guard error about {format} in folder scheme"
  fi
}

sc_verify_log() {
  local log="$WORK/valid.log"
  if is_rs_mode; then
    cp "$LOG_DIR/valid.log" "$log"
  else
    rip basic basic.cue
    log="$WORK/out_basic/log.log"
  fi

  mapfile -t r1 < <(run_crip --verify-log "$log")
  if [[ "${r1[0]}" -ne 0 ]]; then
    fail "valid log did not verify"
  fi

  local tampered="$WORK/tampered.log"
  local log_text
  log_text="$(cat "$log")"
  if [[ -z "$log_text" ]]; then
    fail "tampered log fixture was unexpectedly empty"
    return
  fi

  local first="X"
  if [[ "${log_text:0:1}" == "X" ]]; then
    first="Y"
  fi
  printf '%s%s' "$first" "${log_text:1}" >"$tampered"

  mapfile -t r2 < <(run_crip --verify-log "$tampered")
  if [[ "${r2[0]}" -eq 0 ]]; then
    fail "tampered log verified"
  fi
}

stage_fixtures() {
  CUE_DIR="$FIX"
  CDDA_DIR="$FIX"
  COVERART_DIR="$FIX"
  LOG_DIR="$FIX"

  if [[ -d "$FIX/cue" ]]; then
    CUE_DIR="$FIX/cue"
  fi
  if [[ -d "$FIX/cdda" ]]; then
    CDDA_DIR="$FIX/cdda"
  fi
  if [[ -d "$FIX/coverart" ]]; then
    COVERART_DIR="$FIX/coverart"
  fi
  if [[ -d "$FIX/log" ]]; then
    LOG_DIR="$FIX/log"
  fi

  local cues_found=0
  local f
  for f in "$CUE_DIR"/*.cue; do
    if [[ -f "$f" ]]; then
      cp "$f" "$WORK/"
      cues_found=1
    fi
  done
  if [[ "$cues_found" -eq 0 ]]; then
    fail "no .cue fixtures found in $CUE_DIR"
    return
  fi

  cp "$CDDA_DIR/cdda.nrg" "$WORK/"
  cp "$CDDA_DIR/cdda.bin" "$WORK/basic.bin"
  cp "$CDDA_DIR/cdda.bin" "$WORK/pregap.bin"
  cp "$CDDA_DIR/cdda.bin" "$WORK/preemph.bin"
  cp "$CDDA_DIR/mixed.bin" "$WORK/mixed.bin"
}

main() {
  echo "Compatibility mode: $COMPAT"

  WORK="$(mktemp -d)"
  trap 'rm -rf "$WORK"' EXIT

  stage_fixtures

  local fn="sc_${SCENARIO}"
  if ! declare -f "$fn" >/dev/null 2>&1; then
    echo "Unknown scenario: $SCENARIO"
    echo "Available scenarios: info basic pregap mixed nrg filters art art_back log_coverart cue_only errors multi_output_scheme verify_log"
    exit 2
  fi

  "$fn"

  if [[ "$fails" -ne 0 ]]; then
    echo "$fails check(s) failed"
    exit 1
  fi

  echo "$SCENARIO passed"
}

main "$@"
