#!/usr/bin/env bash
set -euo pipefail

echo "Checking pkg-config entries..."
for lib in libcdio libcdio_cdda libcdio_paranoia; do
  if pkg-config --exists "$lib"; then
    echo "  $lib: $(pkg-config --modversion "$lib")"
  else
    echo "  $lib: missing"
    exit 1
  fi
done

echo
echo "Checking Rust backend feature compilation..."
cargo check --features "backend-libcdio-sys paranoia"

echo
echo "CDDA stack check passed."
echo "Run hardware TOC/frame regression tests with:"
echo "  CYANRIP_CDROM_DEVICE=/dev/sr0 cargo test --features 'backend-libcdio-sys paranoia' --test linux_physical_drive_validation -- --ignored"
echo "Prerequisite: insert a readable audio CD beforehand."
