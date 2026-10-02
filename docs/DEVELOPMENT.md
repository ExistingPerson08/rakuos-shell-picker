# Toolchain & Lingkungan Development

Semua fakta di dokumen ini diverifikasi di mesin ini pada **2026-10-02**.
Kalau angkanya berbeda dengan mesinmu, perbarui dokumen ini, jangan asal
menyesuaikan kode.

Cek cepat sebelum mulai kerja:

```bash
./scripts/check-deps.sh
```

## ⚠️ Penting: `rpm -q` buta terhadap overlay

Toolchain ini dipasang ke **overlay**, dan `rum` menyimpannya di rpmdb yang
terpisah dari base image. Dua rpmdb itu ada berdampingan:

```
$ rum paths
mode: split (RakuOS overlay)
base snapshot rpmdb: /usr/share/rpm
overlay rpmdb:       /var/lib/rakuos/rum-rpmdb
```

`rpm -q` membaca base saja, jadi paket yang ada di overlay terlihat "tidak
terpasang" padahal file-nya utuh di `/usr/bin` dan `/usr/lib64/pkgconfig`.
Ini bukan rpmdb rusak dan bukan pencatatan yang hilang; transactasinya masih
ada, di tempat lain:

```
$ rum history list
  ID  Date and time        Action(s)    Altered
   9  2026-10-02 10:10:31  Install           82
  10  2026-10-02 10:11:58  Install            2
```

Cara yang benar untuk bertanya "apakah paket ini terpasang":

```
$ rum origin rust          ->  rust: Overlay
$ rpm --dbpath /var/lib/rakuos/rum-rpmdb -q rust
rust-1.98.1-1.fc44.x86_64
```

Jangan pernah menyimpulkan "tidak terpasang" dari `rpm -q` saja.

```
rpm -q rust          -> package rust is not installed
rpm -q cargo         -> package cargo is not installed
rpm -q clippy        -> package clippy is not installed
rpm -q rustfmt       -> package rustfmt is not installed
rpm -q gtk4-devel    -> package gtk4-devel is not installed
rpm -qf /usr/bin/rustc        -> file is not owned by any package
rpm -qf /usr/lib64/pkgconfig/gtk4.pc -> file is not owned by any package
```

Jadi: file ada, catatannya ada, tapi di rpmdb yang berbeda. Yang tidak ada
adalah paket yang hilang — overhaul image tidak destroying toolchain selama
overlay masih menyimpan transactasinya.

Yang **masih** tercatat dengan benar dan aman:

```
gtk4-4.22.5-2.fc44          libgtk-4.so.1
libadwaita-1.9.4-1.fc44     libadwaita-1.so.0
librsvg2-2.62.3-1.fc44
gdk-pixbuf2-2.44.6
```

### Apa artinya secara praktis

| Situasi | Akibat |
|---|---|
| `cargo build` hari ini | **Berhasil.** Semua file lengkap ada |
| `dnf install <paket>` lagi | Menginstal ulang, menimpa file yang sama. Tidak merusak |
| `dnf update` | Tidak meng-upgrade toolchain — rpmdb tidak tahu ada toolchain |
| `dnf remove rust` | Gagal / tidak berefek, karena paket tidak dianggap terpasang |
| `bootc upgrade`, ganti image, rpmdb dibangun ulang | **Berisiko.** Kalau image baru tidak menyebut paket-paket ini, toolchain hilang total |

### Yang harus dilakukan

1. **Jangan dihapus manual** `/usr/bin/rustc`, `/usr/bin/cargo`, atau file `.pc`
   di `/usr/lib64/pkgconfig/`. Tidak ada paket yang bisa memulihkannya kalau hilang.
2. Untuk mengecek keberadaan paket, jangan pakai `rpm -q`. Pakai `rum origin <paket>`.
   Lihat `docs/SAFETY.md` untuk versi lengkapnya.
3. Kalau sebuah image baru tidak menyebut toolchain di dalamnya, toolchain itu
   hilang dari base image — tapi masih bisa dipulihkan dari overlay selama
   transactasinya belum di-reset.

## Yang terverifikasi sekarang

| Tool | Versi | Dipakai untuk |
|---|---|---|
| rustc | 1.98.1 (Fedora 1.98.1-1.fc44) | compile |
| cargo | 1.98.1 | build, test, run |
| rustfmt | 1.9.0 | `cargo fmt` |
| clippy | 0.1.98 | `cargo clippy` |
| GTK | 4.22.5 | widget & render SVG |
| libadwaita | 1.9.4 | `ApplicationWindow` |
| glib/gio | 2.88.3 | `gdk::Texture`, `gio::File` |
| cairo | 1.18.4 | render |
| pango | 1.57.1 | teks, ellipsize |
| librsvg | 2.62.3 | render logo SVG saat runtime |
| pkg-config | — | mencari `.pc` |
| rum | 0.1.0 | deteksi paket terpasang, lewat `rum origin` (fitur C2) |
| git | 2.55.0 | version control |

Minimum yang harus dipenuhi: **rustc ≥ 1.80**, **GTK ≥ 4.14** (dibutuhkan
`gdk::Texture::from_file` untuk SVG), **libadwaita ≥ 1.4**.

## Build butuh apa

`cargo build` untuk crate ini butuh:

1. **Toolchain Rust**: `rustc`, `cargo`, `rustfmt`, `cargo-clippy`
2. **File `.pc`**: `gtk4.pc`, `libadwaita-1.pc`, `glib-2.0.pc`, `gio-2.0.pc`,
   `cairo.pc`, `pango.pc`, `gobject-2.0.pc`
3. Tidak ada build script di `Cargo.toml`, jadi tidak butuh compiler C atau
   header untuk crate ini sendiri.

Perintah standar:

```bash
cargo build                  # debug
cargo test                   # unit test, tanpa display
cargo clippy --all-targets
cargo fmt --all
cargo run                    # jalankan picker
```

## Runtime butuh apa

| Kebutuhan | Alasan | Tanpa ini |
|---|---|---|
| Wayland + compositor berjalan | app butuh display | gagal start |
| `rum` | mendeteksi shell terpasang | `rpm -q` hanya melihat base image, sehingga shell di overlay selalu terbaca "not installed" |
| `/proc` | mendeteksi proses berjalan | status "running" tak pernah muncul |
| `~/.config/hypr/config/windowrules.lua` berisi rule ShellPicker | window ikut di-tile tanpa rule | rasio persegi hilang |

Rule yang dibutuhkan:

```lua
hl.window_rule({ match = { class = "^(org\\.rakuos\\.ShellPicker)$" }, float = true, size = { "806", "806" } })
```

## librsvg: tidak ada file `.pc`, dan itu normal

`librsvg-2.0.pc` tidak ada di mesin ini. **Bukan masalah.** Crate ini tidak
memakai librsvg saat build — SVG dimuat lewat GDK saat runtime, dan itu sudah
terbukti bekerja (keempat logo tampil).

Jangan tambahkan crate yang butuh librsvg di build time "supaya aman" — justru
itu yang akan menggagalkan build.

## Tool opsional

Belum terpasang di mesin ini. Berguna, tidak wajib:

| Tool | Paket Fedora | Gunanya |
|---|---|---|
| rust-analyzer | `rust-analyzer` | autocomplete, goto-def, inline diagnostic |
| gdb | `gdb` | backtrace saat crash |
| gtk4-demo | `gtk4-demo` | membandingkan widget GTK secara visual |
| python3-gobject | sudah terpasang | inspeksi widget dari script |
| ImageMagick | sudah terpasang | cek batas artwork SVG |

## Kalau harus pasang ulang

```bash
sudo dnf install -y rustfmt clippy rust cargo gtk4-devel libadwaita-devel
```

Lalu pastikan tercatat benar:

```bash
for p in rust cargo clippy rustfmt gtk4-devel libadwaita-devel; do
  rpm -q "$p" || echo "GAGAL: $p tidak tercatat"
done
./scripts/check-deps.sh
```

Kalau `rpm -q` bilang tidak terpasang padahal file-nya ada, berarti rpmdb
lagi tidak konsisten — bukan berarti pemasangan gagal.

## Aturan

1. `./scripts/check-deps.sh` harus hijau sebelum mulai coding.
2. Kalau menambah dependency, catat alasannya, dan pastikan
   `cargo clippy --all-targets` tetap bersih.
3. Jangan menambah crate yang butuh library sistem baru di build time tanpa
   mengupdate bagian "Build butuh apa" di dokumen ini.
4. Kalau environment berubah (toolchain, versi GTK), perbarui tabel di atas
   beserta tanggal verifikasinya.
5. Angka di [UI.md](UI.md) dan [FEATURES.md](FEATURES.md) harus tetap sinkar
   dengan kode.
