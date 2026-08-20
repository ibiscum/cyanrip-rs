#!/usr/bin/env bash
set -euo pipefail

DEVICE="${CYANRIP_CDROM_DEVICE:-/dev/cdrom}"
FEATURES="backend-libcdio-sys paranoia"

echo "M6 hardware validation using device: $DEVICE"
echo "Prerequisite: insert a readable audio CD beforehand."

./scripts/check_linux_cdda_stack.sh

echo
echo "Running TOC/frame/paranoia/interruption hardware tests..."
CYANRIP_CDROM_DEVICE="$DEVICE" cargo test --features "$FEATURES" --test linux_physical_drive_validation \
  reads_audio_cd_toc_from_real_drive -- --ignored

CYANRIP_CDROM_DEVICE="$DEVICE" cargo test --features "$FEATURES" --test linux_physical_drive_validation \
  reads_one_audio_frame_from_real_drive -- --ignored

CYANRIP_CDROM_DEVICE="$DEVICE" cargo test --features "$FEATURES" --test linux_physical_drive_validation \
  runs_paranoia_pipeline_on_real_drive -- --ignored

CYANRIP_CDROM_DEVICE="$DEVICE" cargo test --features "$FEATURES" --test linux_physical_drive_validation \
  interruption_request_aborts_paranoia_pipeline_on_real_drive -- --ignored

echo
echo "Optional manual media-change scenario:"
echo "  CYANRIP_CDROM_DEVICE=$DEVICE CYANRIP_RUN_MANUAL_MEDIA_CHANGE=1 cargo test --features '$FEATURES' --test linux_physical_drive_validation manual_media_change_scenario_reference -- --ignored"
echo "See docs/M6_REAL_HARDWARE_VALIDATION.md for acceptance notes template."
