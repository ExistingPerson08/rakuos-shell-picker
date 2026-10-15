#!/usr/bin/env bash
# Build + seluruh pemeriksaan otomatis rakuos-shell-picker.
#
#   ./scripts/build-test.sh
#
# Urutannya: build, test, clippy. Menjalankan ketiganya walau ada yang gagal,
# supaya satu kali jalan menunjukkan semua masalah, lalu keluar != 0 kalau ada
# yang merah. Test yang `ignored` tidak ikut jalan — dua di antaranya menulis
# config user asli.

set -uo pipefail

cd "$(dirname "$0")/.." || exit 2

fail=0
step() { printf '\n== %s ==\n' "$1"; }

step "cargo build"
cargo build || fail=1

step "cargo test"
cargo test || fail=1

step "cargo clippy --all-targets"
cargo clippy --all-targets || fail=1

echo
if [ "$fail" -ne 0 ]; then
  echo "ADA YANG GAGAL" >&2
else
  echo "SEMUA HIJAU"
fi
exit "$fail"
