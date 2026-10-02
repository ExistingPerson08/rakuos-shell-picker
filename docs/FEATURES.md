# Fitur

Status: **selesai** = jalan & teruji · **sebagian** = UI ada, fungsional belum ·
**belum** = belum dikerjakan · **keluar scope** = sengaja tidak.

## A. Fondasi

| # | Fitur | Status |
|---|---|---|
| A1 | Binary `rakuos-shell-picker`, app ID `org.rakuos.ShellPicker` | selesai |
| A2 | Satu crate (`lib` + `bin`) dengan modul per compositor | selesai |
| A3 | Struktur modular, `ui/` terisolasi dari backend | selesai |
| A4 | 10 unit test (registry, logo, urutan, state) | selesai |
| A5 |Clippy bersih, `cargo test` hijau | selesai |

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
| B9 | Placeholder "Coming soon" untuk compositor tanpa shell | selesai |
| B10 | Kartu `unavailable` untuk shell yang belum terpasang | selesai |
| B11 | Konfirmasi sebelum apply | belum |
| B12 | Tombol Refresh / Install | belum |

## C. Deteksi

| # | Fitur | Status |
|---|---|---|
| C1 | Binary ada di `PATH` | selesai |
| C2 | Paket terpasang (`rpm -q`) | selesai |
| C3 | Proses sedang jalan (scan `/proc/*/comm`) | selesai |
| C4 | Status `running` / `installed` / `not installed` di kartu | selesai |
| C5 | Health probe per shell (socket/API) | belum |

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
| E11 | Registry shell untuk Niri | belum |
| E12 | Cek environment sebelum switch (display, Wayland) | belum |

## Keluar scope

| Item | Alasan |
|---|---|
| Mengganti compositor (Hyprland → Niri) | Untuk ini seharusnya pakai TTY/session manager, bukan picker |
| Installer paket | Paket datang dari image; picker tidak memanggil yum/dnf |
| Mengedit isi konfigurasi | Picker memilih, bukan editor |
| Window manager-style tiling | Bukan WM, dan WM yang memegang layout |
| Config preset / generator | Berisiko menimpa file user |

## Rencana

Urutan pengerjaan:

1. **Kunci UI.** Setuju dengan tampilan; inisialisasi git; commit. Supaya semua
   perubahan backend nanti punya titik balik yang jelas.
2. **`ShellBackend`.** trait + implementasi Noctalia duluan sebagai contoh.
   Semua backend berikutnya mengikuti pola yang sama.
3. **Backup + rollback.** Ini yang pertama, sebelum ada switch sama sekali. Lihat
   [SAFETY.md](SAFETY.md).
4. **Switch end-to-end.** Stop → start → verifikasi → tulis state. Uji dengan
   DMS karena itu satu-satunya yang terpasang di mesin ini.
5. **Niri.** Registri shell terpisah, dari nol.

## Syarat "selesai" per fitur

Fitur dianggap selesai kalau:

- [ ] Ada test otomatisnya, atau manual step yang bisa diulang orang lain.
- [ ] Tidak melanggar satu pun aturan di [SAFETY.md](SAFETY.md).
- [ ] `cargo test` dan `cargo clippy --all-targets` hijau.
- [ ] [UI.md](UI.md) atau dokumen ini diperbarui kalau ada perilaku/angka yang
      berubah.
- [ ] Dicatat di sini kalau Status-nya berubah.
