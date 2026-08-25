#!/usr/bin/env bash
set -euo pipefail

DEVICE="${CYANRIP_CDROM_DEVICE:-/dev/cdrom}"
EXPECTED_DISCID="${CYANRIP_EXPECT_MULTI_RELEASE_DISCID:-BKkzOxbdODYWFIOEEZ3b.b_nm64-}"
FEATURES="backend-libcdio-sys paranoia cdda"

echo "M7 info-only release disambiguation validation"
echo "Device: $DEVICE"
echo "Expected DiscID: $EXPECTED_DISCID"
echo "Prerequisites:"
echo "  1) Insert the same multi-release audio CD used for fixture capture"
echo "  2) Network access to MusicBrainz"

./scripts/check_linux_cdda_stack.sh

echo
echo "Running -I disambiguation test with -R 1..."
CYANRIP_CDROM_DEVICE="$DEVICE" \
CYANRIP_EXPECT_MULTI_RELEASE_DISCID="$EXPECTED_DISCID" \
cargo test --features "$FEATURES" --test run_workflow_cli \
  info_only_mode_with_release_index_1_disambiguates_musicbrainz_result -- --ignored

echo
echo "Running -I disambiguation test with -R 2..."
CYANRIP_CDROM_DEVICE="$DEVICE" \
CYANRIP_EXPECT_MULTI_RELEASE_DISCID="$EXPECTED_DISCID" \
cargo test --features "$FEATURES" --test run_workflow_cli \
  info_only_mode_with_release_index_2_disambiguates_musicbrainz_result -- --ignored

echo
echo "M7 info-only release disambiguation validation completed."
