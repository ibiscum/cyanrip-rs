#!/usr/bin/env bash
set -euo pipefail

# Generate CD image fixtures from a directory of audio samples.
# Port of upstream tests/gen_fixtures.py behavior.
#
# Usage:
#   ./scripts/gen_fixtures.sh <samples-dir> [fixtures-root]
#
# Output (default fixtures-root: tests/fixtures):
#   tests/fixtures/cdda/cdda.bin
#   tests/fixtures/cdda/mixed.bin
#   tests/fixtures/cdda/cdda.nrg
#   tests/fixtures/coverart/art.png

usage() {
  echo "Usage: $0 <samples-dir> [fixtures-root]"
  echo ""
  echo "Requires: ffmpeg, perl"
  echo "Samples: any decodable audio files; needs at least 6 sorted files."
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ $# -lt 1 || $# -gt 2 ]]; then
  usage
  exit 2
fi

SAMPLES_DIR="$1"
FIX_ROOT="${2:-tests/fixtures}"

if [[ ! -d "$SAMPLES_DIR" ]]; then
  echo "samples dir does not exist: $SAMPLES_DIR"
  exit 1
fi

if ! command -v ffmpeg >/dev/null 2>&1; then
  echo "ffmpeg not found in PATH"
  exit 1
fi

if ! command -v perl >/dev/null 2>&1; then
  echo "perl not found in PATH"
  exit 1
fi

SECTOR=2352
SECTORS_PER_SEC=75
PREGAP_SECTORS=150

CDDADIR="$FIX_ROOT"
COVERDIR="$FIX_ROOT"
if [[ -d "$FIX_ROOT/cdda" ]]; then
  CDDADIR="$FIX_ROOT/cdda"
fi
if [[ -d "$FIX_ROOT/coverart" ]]; then
  COVERDIR="$FIX_ROOT/coverart"
fi

mkdir -p "$CDDADIR" "$COVERDIR"

mapfile -t SAMPLES < <(find "$SAMPLES_DIR" -maxdepth 1 -type f | LC_ALL=C sort)
if [[ ${#SAMPLES[@]} -lt 6 ]]; then
  echo "need at least 6 sample files in sorted order, found ${#SAMPLES[@]}"
  exit 1
fi

WORKDIR="$(mktemp -d)"
cleanup() {
  rm -rf "$WORKDIR"
}
trap cleanup EXIT

decode_segment() {
  local sample="$1"
  local seek="$2"
  local seconds="$3"
  local out_file="$4"

  local nb_bytes=$((seconds * SECTORS_PER_SEC * SECTOR))

  ffmpeg -v error -ss "$seek" -i "$sample" -f s16le -ar 44100 -ac 2 -y "$out_file" >/dev/null 2>&1

  local got
  got="$(wc -c <"$out_file")"
  if (( got < nb_bytes )); then
    echo "$sample: too short, wanted ${seconds}s at ${seek}s"
    exit 1
  fi

  if (( got > nb_bytes )); then
    truncate -s "$nb_bytes" "$out_file"
  fi
}

SEG1="$WORKDIR/seg1.pcm"
SEG2="$WORKDIR/seg2.pcm"
SEG3="$WORKDIR/seg3.pcm"
AUDIO="$WORKDIR/audio.pcm"

decode_segment "${SAMPLES[1]}" 5 3 "$SEG1"
decode_segment "${SAMPLES[3]}" 10 3 "$SEG2"
decode_segment "${SAMPLES[5]}" 15 2 "$SEG3"
cat "$SEG1" "$SEG2" "$SEG3" >"$AUDIO"

expected_audio_bytes=$((600 * SECTOR))
actual_audio_bytes="$(wc -c <"$AUDIO")"
if (( actual_audio_bytes != expected_audio_bytes )); then
  echo "unexpected audio size: got $actual_audio_bytes expected $expected_audio_bytes"
  exit 1
fi

cp "$AUDIO" "$CDDADIR/cdda.bin"

perl - "$AUDIO" "$CDDADIR/mixed.bin" <<'PERL'
use strict;
use warnings;

my ($audio_path, $out_path) = @ARGV;
my $SECTOR = 2352;
my $SECTORS_PER_SEC = 75;
my $PREGAP_SECTORS = 150;

open my $afh, '<:raw', $audio_path or die "open $audio_path: $!";
my $audio = do { local $/; <$afh> };
close $afh;

sub bcd {
    my ($v) = @_;
    return (($v / 10) << 4) | ($v % 10);
}

sub mode1_sector {
    my ($lba, $payload) = @_;

    my $abs = $lba + $PREGAP_SECTORS;
    my $m = int($abs / (60 * $SECTORS_PER_SEC));
    my $rem = $abs % (60 * $SECTORS_PER_SEC);
    my $s = int($rem / $SECTORS_PER_SEC);
    my $f = $rem % $SECTORS_PER_SEC;

    my $sync = "\x00" . ("\xFF" x 10) . "\x00";
    my $header = pack('C4', bcd($m), bcd($s), bcd($f), 0x01);
    my $data = substr($payload, 0, 2048);
    $data .= "\x00" x (2048 - length($data));

    my $sector = $sync . $header . $data;
    $sector .= "\x00" x ($SECTOR - length($sector));
    return $sector;
}

my $out = '';
for my $i (0..149) {
    my $payload = (sprintf("CYANRIP TEST DATA sector %03d ", $i)) x 64;
    $out .= mode1_sector($i, $payload);
}
$out .= substr($audio, 0, 300 * $SECTOR);

open my $ofh, '>:raw', $out_path or die "open $out_path: $!";
print {$ofh} $out;
close $ofh;
PERL

perl - "$AUDIO" "$CDDADIR/cdda.nrg" <<'PERL'
use strict;
use warnings;

my ($audio_path, $out_path) = @ARGV;
my $SECTOR = 2352;
my $PREGAP_SECTORS = 150;

open my $afh, '<:raw', $audio_path or die "open $audio_path: $!";
my $audio = do { local $/; <$afh> };
close $afh;

sub bcd {
    my ($v) = @_;
    return (($v / 10) << 4) | ($v % 10);
}

sub cuex_entry {
    my ($track, $index, $lsn) = @_;
    my $trk = ($track == 0xAA) ? 0xAA : bcd($track);
    return pack('C C C C l>', 0x01, $trk, $index, 0, $lsn);
}

sub chunk {
    my ($cid, $payload) = @_;
    return $cid . pack('N', length($payload)) . $payload;
}

my ($t1_start, $t2_pregap, $t2_start, $end) = (0, 150, 225, 450);

my $data = ("\x00" x ($PREGAP_SECTORS * $SECTOR)) . substr($audio, 0, $end * $SECTOR);

my $cuex =
    cuex_entry(1, 0, -$PREGAP_SECTORS) .
    cuex_entry(1, 1, $t1_start) .
    cuex_entry(1, 1, $t1_start) . cuex_entry(2, 1, $t2_start) .
    cuex_entry(2, 1, $t2_start) . cuex_entry(0xAA, 1, $end);

sub file_off {
    my ($lsn) = @_;
    return ($lsn + $PREGAP_SECTORS) * $SECTOR;
}

my @tracks = (
    [0, $t1_start, $t2_pregap],
    [$t2_pregap, $t2_start, $end],
);

my $daox = pack('V', 22 + 42 * scalar(@tracks));
$daox .= "\x00" x 13;
$daox .= pack('C3', 0, 0, 0);
$daox .= pack('C2', 1, scalar(@tracks));

for my $t (@tracks) {
    my ($i0, $i1, $trk_end) = @$t;
    $daox .= "\x00" x 12;
    $daox .= pack('n n n', $SECTOR, 0x0700, 1);
    my $i0_off = $i0 ? file_off($i0) : 0;
    $daox .= pack('Q> Q> Q>', $i0_off, file_off($i1), file_off($trk_end));
}

my $footer =
    chunk('CUEX', $cuex) .
    chunk('DAOX', $daox) .
    chunk('SINF', pack('N', scalar(@tracks))) .
    chunk('MTYP', pack('N', 1)) .
    chunk('END!', '');

my $nrg = $data . $footer . 'NER5' . pack('Q>', length($data));

open my $ofh, '>:raw', $out_path or die "open $out_path: $!";
print {$ofh} $nrg;
close $ofh;
PERL

ffmpeg -v error -y -f lavfi -i color=red:size=8x8 -frames:v 1 "$COVERDIR/art.png" >/dev/null 2>&1

echo "fixtures written:"
echo "  $CDDADIR/cdda.bin"
echo "  $CDDADIR/mixed.bin"
echo "  $CDDADIR/cdda.nrg"
echo "  $COVERDIR/art.png"
