# Fitur

Status: **selesai** = jalan & teruji · **sebagian** = UI ada, fungsional belum ·
**belum** = belum dikerjakan · **keluar scope** = sengaja tidak.

## A. Fondasi

| # | Fitur | Status |
|---|---|---|
| A1 | Binary `rakuos-shell-picker`, app ID `org.rakuos.ShellPicker` | selesai |
| A2 | Satu crate (`lib` + `bin`) dengan modul per compositor | selesai |
| A3 | Struktur modular, `ui/` terisolasi dari backend | selesai |
| A4 | 27 unit test (registry, logo, urutan, state, guard, dialog) | selesai |
| A5 | Clippy bersih, `cargo test` hijau | selesai |

## B. Tampilan

| # | Fitur | Status |
|---|---|---|
| B1 | Window persegi 806×806, float, rule Hyprland | selesai |
| B2 | Tema gelap, panel semi-transparan, outline 1px | selesai |
| B3 | Tab compositor (Hyprland / Niri) dari registry | selesai |
| B4 | Grid 4 kolom, lebar kartu dipatok 150px | selesai |
| B5 | Grid scroll vertikal, lebar terkunci (aman 20+ shell) | selesai |
| B6 | Logo SVG langsung via GdkTexture, tanpa PNG | selesai |
| B7 | Logo & label center, nama panjang di-ellipsize | selesai |
| B8 | Panel log 100px di bawah, monospace, scroll sendiri | selesai |
| B13 | Status `running` diberi warna hijau | selesai |
| B14 | Label aksi berbunyi "switch", bukan "install" | selesai |
| B15 | Panel log hanya tampil saat ada operasi, hidden saat idle | selesai |
| B16 | Panel log gagal tetap tampil sampai ditutup | selesai (badge di card belum) |
| B17 | Halaman compositor yang tidak sedang jalan menampilkan banner + penolakan saat diklik | selesai |
| B9 | Placeholder "Coming soon" untuk compositor tanpa shell | selesai (niri sudah punya isi) |
| B10 | Kartu `unavailable` untuk shell yang belum terpasang | selesai |
| B11 | Dialog konfirmasi Yes/No sebelum apply | selesai (UI; eksekusi menyusul) |
| B12 | Tombol Refresh / Install | belum |
| B18 | Kanal log terpisah: file aplikasi + panel UI (bukan satu panel untuk semua) | selesai |

## C. Deteksi

| # | Fitur | Status |
|---|---|---|
| C1 | Binary ada di `PATH` | selesai |
| C2 | Paket terpasang (`rpm -q`) | selesai |
| C3 | Proses sedang jalan (scan `/proc/*/comm`) | selesai |
| C4 | Status `running` / `switch` / `not installed` di kartu | selesai |
| C5 | Health probe per shell (socket/API) | belum |
| C6 | Deteksi compositor aktif dari envvar (`HYPRLAND_INSTANCE_SIGNATURE`, `NIRI_SOCKET`) | selesai |
| C7 | Deteksi sesi Wayland (`WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`) | selesai |
| C8 | Fallback deteksi lewat `/proc/*/exe` | selesai |
| C9 | Tolak semua aksi kalau compositor tidak terdeteksi | selesai |

## D. State

| # | Fitur | Status |
|---|---|---|
| D1 | Simpan `active` / `previous` di XDG state | selesai |
| D2 | `record_switch`, lookup `active_shell` | selesai |
| D3 | Backups dir disiapkan | selesai |
| D4 | Tampilkan shell aktif dari state di UI | belum |
| D5 | Tulis state secara atomik (temp file + rename) | belum |
| D6 | Simpan compositor aktif, bukan cuma shell | belum |

## E. Backend — belum dimulai

Ini inti dari proyek, dan sengaja ditahan sampai UI dikunci.

| # | Fitur | Status |
|---|---|---|
| E1 | Trait `ShellBackend` (start/stop/status/backup) | belum |
| E2 | Backup konfigurasi sebelum switch | belum |
| E3 | Rollback kalau switch gagal | belum |
| E4 | Backend Noctalia | belum |
| E5 | Backend DMS | belum |
| E6 | Backend Caelestia | belum |
| E7 | Backend Ambxst | belum |
| E8 | Stop shell lama dengan rapi (bukan `kill -9`) | belum |
| E9 | Start shell baru dengan mekanisme yang benar | belum |
| E10 | Deteksi shell yang jalan tanpa lewat daftar kita | belum |
| E11 | Registry shell untuk Niri (Noctalia, DMS, Ambxst) | selesai |
| E12 | Cek environment sebelum switch (display, Wayland) | selesai (UI) |
| E13 | Guard di backend: `preflight()` menolak cross-compositor | sebagian (UI menolak, `preflight()` belum) |

## G. Logging

Spesifikasi lengkap di [LOGGING.md](LOGGING.md).

| # | Fitur | Status |
|---|---|---|
| G1 | Log aplikasi ke `XDG_RUNTIME_DIR`, 0600, dipotong tiap start | belum |
| G2 | Fallback `/tmp/rakuos-shell-picker-$UID/` mode 0700 | belum |
| G3 | Log gagal per waktu di `state/logs/<shell>/<timestamp>.log`, 0600 | belum |
| G4 | Tidak ada log file saat switch sukses | belum |
| G5 | Rotasi 10 kegagalan terakhir per shell | belum |
| G6 | `operation_id` mengorelasikan 3 saluran log | belum |
| G7 | `metadata.toml` per kegagalan | belum |
| G8 | Larangan: jangan pernah tulis isi config atau token ke log | belum |

## Keluar scope

| Item | Alasan |
|---|---|
| Mengganti compositor (Hyprland → Niri) | Berarti ganti image; urusan rakuos system management. Ditolak oleh guard, lihat [SAFETY.md](SAFETY.md) |
| Installer paket | Paket datang dari image; picker tidak memanggil dnf |
| Installer paket | Paket datang dari image; picker tidak memanggil yum/dnf |
| Mengedit isi konfigurasi | Picker memilih, bukan editor |
| Window manager-style tiling | Bukan WM, dan WM yang memegang layout |
| Config preset / generator | Berisiko menimpa file user |

## Rencana

Urutan pengerjaan:

1. **Kunci UI.** Setuju dengan tampilan; inisialisasi git; commit. Supaya semua
   perubahan backend nanti punya titik balik yang jelas. *(selesai)*
2. **Deteksi + guard.** `Compositor::is_running()`, `compositor::current()`,
   penolakan cross-compositor di UI **dan** di `preflight()`.
3. **Logging.** Tiga saluran sesuai [LOGGING.md](LOGGING.md), panel on-demand.
4. **Backup + rollback.** Ini duluan, sebelum ada switch sama sekali, supaya
   tidak ada kode yang bisa merusak sebelum ada jaring pengaman. Lihat
   [SAFETY.md](SAFETY.md).
5. **Switch end-to-end.** Stop → start → verifikasi → tulis state. Uji dengan
   DMS karena itu satu-satunya yang terpasang di mesin ini.
6. **Niri.** Registri shell terpisah, dari nol.

## Syarat "selesai" per fitur

Fitur dianggap selesai kalau:

- [ ] Ada test otomatisnya, atau manual step yang bisa diulang orang lain.
- [ ] Tidak melanggar satu pun aturan di [SAFETY.md](SAFETY.md).
- [ ] `cargo test` dan `cargo clippy --all-targets` hijau.
- [ ] [UI.md](UI.md) atau dokumen ini diperbarui kalau ada perilaku/angka yang
      berubah.
- [ ] Dicatat di sini kalau Status-nya berubah.
