#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ESP_DIR="${ROOT_DIR}/esp"

echo "--- [STEP 1] Building Kernel (osmrx) ---"
# We enter the directory so Cargo picks up the local .cargo/config.toml
# and the unstable build-std settings.
pushd osmrx >/dev/null
cargo build
popd >/dev/null

echo "--- [STEP 2] Building Bootloader (bootmrx) ---"
pushd bootmrx >/dev/null
cargo build
popd >/dev/null

# Define paths (Now pointing to the unified workspace target folder)
KERNEL_BIN="${ROOT_DIR}/target/x86_64-osmrx/debug/osmrx"
BOOTLOADER_EFI="${ROOT_DIR}/target/x86_64-unknown-uefi/debug/bootmrx.efi"

echo "--- [STEP 3] Preparing ESP ---"
mkdir -p "${ESP_DIR}/EFI/BOOT"
cp "${BOOTLOADER_EFI}" "${ESP_DIR}/EFI/BOOT/BOOTX64.EFI"
cp "${KERNEL_BIN}" "${ESP_DIR}/kernel.elf"

echo "--- [STEP 4] Launching QEMU ---"
qemu-system-x86_64 \
  -bios "${ROOT_DIR}/OVMF_CODE.fd" \
  -drive "file=fat:rw:${ESP_DIR},format=raw" \
  -net none \
  -vga std \
  -debugcon stdio \
  -global isa-debugcon.iobase=0xe9 \
  -no-reboot \
  -m 256M
