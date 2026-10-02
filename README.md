# rakuos-shell-picker

Pilih **desktop shell** (panel/widgets/status layer) yang berjalan di atas
sebuah compositor. Compositor sendiri — Hyprland, Niri — **tidak** berganti:
selama window picker terbuka, compositor yang sedang jalan tetap jalan.

```
Hyprland / Niri        <- compositor, selalu hidup, tidak pernah di-restart
   └── shell           <- yang dipilih user: Noctalia, DMS, Caelestia, Ambxst
```

## Status

Prototype. UI sudah jadi dan berjalan; pergantian shell sungguhan belum
diimplementasikan. Lihat [docs/FEATURES.md](docs/FEATURES.md) untuk daftar
lengkap fitur beserta statusnya.

## Build & run

```bash
cd "/var/home/mindset/Project/DE switch/rakuos-shell-picker"
cargo run -p rakuos-shell-picker   # atau: cargo run
```

Butuh GTK 4.22 dan libadwaita 1.9 — logo dibaca langsung sebagai SVG, tidak ada
aset PNG yang perlu dikonversi.

```bash
cargo test      # registry + session state
cargo clippy --all-targets
```

## Layout project

```
src/main.rs      entry point
src/app.rs       aplikasi GTK & window
src/ui/          widget, satu file per bagian
src/compositor/  satu folder per compositor
  hyprland/shells/   satu file per shell
  niri/
src/shell.rs     tipe Shell & cara memeriksa satu shell
src/session.rs   state active/previous
assets/          logo SVG
docs/            spesifikasi & rencana
```

Aturan singkat: **menambah shell tidak boleh menyentuh `ui/`**. Detailnya di
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Dokumentasi

| Dokumen | Isi |
|---|---|
| [docs/README.md](docs/README.md) | indeks dokumen |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | struktur modul & aturan dependensi |
| [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) | toolchain, dependency, pemeriksaan environment |
| [docs/FEATURES.md](docs/FEATURES.md) | fitur, status, syarat selesai |
| [docs/SAFETY.md](docs/SAFETY.md) | jaminan backup & rollback |
| [docs/UI.md](docs/UI.md) | spesifikasi layout & angka tetap |

Sebelum mulai coding, cek environment:

```bash
./scripts/check-deps.sh
```

## Catatan integrasi

- App ID: `org.rakuos.ShellPicker`
- Binary: `rakuos-shell-picker`
- Window harus float + ukuran tetap agar tidak ikut tile: rule Hyprland ada di
  `~/.config/hypr/config/windowrules.lua`.
- State: `~/.local/state/rakuos/shell-picker/state.toml`
