# Spesifikasi Tampilan

Semua angka di sini adalah nilai yang benar-benar dipakai di `src/ui/`.
Kalau kamu ubah angka, ubah juga file yang disebutnya.

## Window

| Properti | Nilai | Sumber |
|---|---|---|
| Rasio | 1:1 | square, rule Hyprland |
| Ukuran | 806×806 | rule di `~/.config/hypr/config/windowrules.lua` |
| Default app | 800×800 | `app.rs` `DEFAULT_SIZE` |
| Tipe | float, fixed size | rule Hyprland |

Window butuh rule karena Hyprland yang menentukan ukuran window. Tanpa rule,
picker ikut di-tile dan kehilangan rasio persegi.

## Susunan

Vertikal, dari atas:

```
+--------------------------------------------------+
|                                                  |
|              hyprland   niri                     |   tabbar
|                                                  |
|     +-------+  +-------+  +-------+  +------+   |
|     | logo | | | logo | | | logo | | | logo |   |   grid 4 kolom
|     | 100x | | |      | | |      | | |      |   |
|     +------+ +-------+ +-------+ +-------+   |
|     Nama    Nama      Nama     Nama           |
|                                                  |
|     +------+  +------+  +------+  +------+    |
|     ...  (scroll ke bawah, window tidak melar)  |
|                                                  |
|  +--------------------------------------------+  |
|  | log, 100px, monospace                      |  |
|  +--------------------------------------------+  |
+--------------------------------------------------+
```

Tinggi log tetap 100px; sisa tinggi window dipakai grid. Grid yang grows ke
bawah, bukan window.

## Angka tetap

Semua nilai ini di-pin di Rust, bukan di CSS.

| Konstanta | Nilai | File | Alasan |
|---|---|---|---|
| `COLUMNS` | 4 | `ui/grid.rs` | lebar window cukup |
| `COLUMN_SPACING` | 34 | `ui/grid.rs` | jarak kolom |
| `ROW_SPACING` | 46 | `ui/grid.rs` | jarak baris |
| `CARD_WIDTH` | 150 | `ui/card.rs` | kunci lebar kolom |
| `CARD_WIDE` | 320 | `ui/grid.rs` | kartu placeholder |
| `LOG_HEIGHT` | 100 | `ui/log.rs` | ¼ dari tinggi window |
| `LOGO_BOX` | 100 | `ui/logo.rs` | kotak logo |
| `LOGO_SIZE` | 72 | `ui/logo.rs` | panjang sisi artwork |

## Kenapa lebar kartu di Rust

Lebar grid harus terkunci: `4 × 150 + 3 × 34 = 702px`. Kalau lebar kartu
diatur dari CSS atau dari teks nama, grid akan melebar dan baris bisa
menjadi tidak sejajar.

Dua pengaman lain:

- Nama dibatasi `max_width_chars(12)` + ellipsize end. Nama shell yang panjang
  tidak akan menambah lebar kolom.
- Scroller memakai policy horizontal `Never`, jadi konten yang lebih lebar
  dipotong, bukan menggulir ke samping.

Diverifikasi dengan 4 / 12 / 20 kartu (lebar tetap 702px, tinggi 166 / 566 /
966px, viewport ~302px, scroll aktif di 12 dan 20).

## Logo

- SVG dimuat langsung sebagai `gdk::Texture` dari `gio::File`. GTK 4.22 bisa
  merender SVG, jadi tidak ada langkah konversi dan tidak ada PNG fallback.
- Nama file diambil dari `Shell::logo`, dideklarasikan di file shell itu
  sendiri.
- `pixel_size(72)` **dan** `size_request(72, 72)` dua-duanya dipasang.
  `pixel_size` hanya menetapkan batas bawah; tanpa `size_request`, image bisa
  dialokasikan sebesar ukuran natural SVG (mis. 256×256) dan meluber.
- Kalau SVG gagal dimuat, jatuh ke huruf awal (`N`, `D`, `C`, `A`).

## Kartu shell

Isi: kotak logo 100×100, nama, status.

Status memakai **kata kerja "switch"**, bukan "install" — karena aksi utamanya
memindah shell yang sedang berjalan, bukan memasang paket:

| Keadaan | Label | Warna |
|---|---|---|
| Proses hidup | `running` | **hijau** |
| Terpasang, tidak jalan | `switch` | netral |
| Belum terpasang | `switch` + badge `not installed` | diredupkan |
| Compositornya tidak sedang jalan | badge `not the running compositor` | diredupkan + non-interaktif |

- Shell yang belum terpasang dapat class CSS `unavailable`.
- Card milik compositor yang tidak sedang jalan dapat class CSS `disabled`:
  tidak bisa diklik, hover tidak berubah, ada badge penjelas.
- Single-selection: memilih satu kartu mematikan yang lain.
- Card yang gagal dapat class CSS `failed` sampai operasi berikutnya.

## Tab compositor

- Satu tab per entri di `compositor::all()`, urutannya urutan daftar.
- Tab aktif diberi class CSS `active`.
- Konten tiap compositor punya anak `gtk::Stack` dengan nama = `Compositor::id()`,
  transisi crossfade.
- Tab pertama yang aktif saat window dibuka (`compositor::default_id()`).

## Placeholder

Compositor dengan daftar shell kosong menampilkan satu kartu lebar di tengah,
bukan halaman kosong:

```
+--------------------------------+
|              ?                 |
|         Coming soon            |
|  no <id> shells registered yet |
+--------------------------------+
```

Teks dibuat dari `Compositor::id()`, bukan ditulis manual per compositor.

## Panel log

- **Tersembunyi saat idle.** Tidak pernah menampilkan versi aplikasi atau
  daftar shell; itu isi log aplikasi, bukan panel UI.
- Tampil hanya saat ada operasi berjalan.
- Sukses → langsung tersembunyi.
- Gagal → **tetap tampil sampai ditutup**, plus badge di card yang gagal, lalu
  detailnya ditulis ke file log gagal.
- Tinggi tetap 100px, melekat di bawah.
- `gtk::ScrolledWindow` dengan policy horizontal `Never`, vertikal
  `Automatic`.
- Font monospace, read-only, tanpa cursor.
- Ditulis dengan `buffer.insert` di `end_iter`, satu baris per panggilan.
- `log::panel()` mengembalikan **frame**-nya, bukan scroller di dalamnya.
  Versi lama mengembalikan scroller sehingga styling `.log-frame` tidak pernah
  terpakai.

## Palet

| Token | Nilai |
|---|---|
| Background | `#111318` |
| Surface | `#1a1e26` |
| Surface/card | `#222733` |
| Border | `alpha(white, 0.10)` |
| Teks | `#e8eaf0` |
| Teks redup | `#9aa1b1` |
| Aksen | `#7aa2f7` |
| Sukses / `running` | `#9ece6a` |
| Galat / `failed` | `#f7768e` |
| Nonaktif | `#565f89` |

Semua warna lewat class CSS di `src/style.css`.

## Catatan

- Gambar wallpaper blur yang ada di wireframe asli **tidak dipakai**; window
  sekarang warna solid. Kalau wallpaper ditambahkan nanti, tinggi log tetap
  100px dan grid tetap boleh scroll.
- Placeholder `Logo DE` di wireframe lama sudah diganti logo shell sungguhan.
- Wireframe lama menyebut 8 kartu; yang benar 4 kartu per compositor, dan
  jumlah itu boleh bertambah tanpa mengubah layout.
