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
|              hyprland                           |   tabbar
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
| Belum terpasang | `not installed` | diredupkan |

- Shell yang belum terpasang dapat class CSS `unavailable`, dan sub-labelnya
  memakai kata `not installed` — bukan `switch`, karena memang tidak ada yang
  bisa dipindah dan memasang paket berada di luar lingkup picker.
- Shell yang sedang hidup dapat class CSS `running` pada tombol **dan** pada
  sub-label, sehingga teksnya ikut hijau.
- Card milik compositor yang tidak sedang jalan **tetap bisa diklik**, bukan
  non-interaktif. Mengkliknya membuka dialog penolakan, karena card yang dibuat
  mati hanya terlihat rusak.
- Single-selection: memilih satu kartu mematikan yang lain.
- Sorotan hanya menempel kalau hasilnya `Confirm switch`. Kalau ditolak atau
  user menekan Cancel, sorotan dilepas, supaya tidak terlihat seperti ada
  switch yang tertunda.
- Card yang gagal dapat class CSS `failed` sampai operasi berikutnya.

## Dialog safety

`AdwAlertDialog`. Kelima jalur klik, ditentukan oleh fungsi murni
`ui::dialog::decide()` yang sudah ada testnya sendiri:

| Keadaan | Judul | Body | Baris penekanan |
|---|---|---|---|
| Compositor berbeda | `Blocked` | `niri is not the running compositor.` | **niri is running.** Switch image in "RakuOS system management". |
| Compositor tidak terdeteksi | `Blocked` | alasannya | Nothing was changed. |
| Shell sudah hidup | `Already running` | `Noctalia is already running.` | Nothing to switch to. |
| Shell belum terpasang | `Not installed` | `Caelestia Shell is not installed.` | Packages come from the **RakuOS** image. |
| Shell terpasang dan idle | `Switch to X?` | `Noctalia is running now.` | Configuration is backed up and never deleted. |

- **Pendek, bukan paragraf.** Awalnya tiap popup dua paragraf; yang penting
  sudah ada di banner kuning di atas dan di log, jadi mengulanginya di sini
  cuma menambah teks kedaluwarsa. Yang tampil satu kalimat + satu baris
  penekanan.
- `AdwAlertDialog` menampilkan body sebagai teks polos, jadi `<b>` di body akan
  tampil sebagai karakter biasa. Karena itu baris yang ditebalkan ditaruh di
  `extra` sebagai `gtk::Label` dengan `use_markup(true)`.
- Nama produk selalu ditulis dalam tanda kutip: **"RakuOS system management"**,
  supaya jelas itu nama fitur, bukan sekadar kata di dalam kalimat.
- Response ditambahkan **sebelum** `present()`. Versi lama menampilkan dialog
  lebih dulu, sehingga tombolnya belum ada saat dialog muncul.
- Dialog penolakan hanya punya `Close`; tidak ada yes/no karena memang tidak
  ada yang bisa disetujui.
- Dialog switch punya `Cancel` sebagai default response, jadi Enter melakukan
  hal yang aman.
- Hanya `Confirm switch` + `Switch` yang boleh lanjut ke backend. Empat jalur
  lain berhenti di sini, dan itu diuji langsung
  (`every_refusal_is_distinct_from_a_confirmation`).
- Eksekusi switch adalah urusan `ShellBackend::preflight()`, bukan dialog.

## Halaman compositor yang diblokir

Compositor yang bukan yang sedang berjalan tetap menampilkan daftar shell-nya
penuh. Menutupinya tidak membantu: yang perlu dilihat user justru apa yang akan
mereka dapat kalau nanti memang berganti image, dan bagaimana bedanya dengan
compositor sekarang.

- **Banner kuning di paling atas**, sebelum grid, dengan alasan penolakannya.
  Banner ini bukan pesan error, melainkan pemberitahuan lebih awal.
- **Kartu tetap tampil dan tetap bisa diklik.** Mengklik salah satu memunculkan
  dialog penolakan, karena itu tujuan akhirnya. Kartu yang dibuat mati hanya
  terlihat rusak.
- Banner juga muncul di halaman yang benar-benar kosong, di atas placeholder.

Hanya ada satu halaman, jadi tabel ini yang tersisa dari perbandingan antar
compositor. Niri pernah jadi kolom kedua dan sudah dihapus.

| Shell | hyprland | catatan |
|---|---|---|
| Noctalia | ya | di base image |
| DankMaterialShell | ya | satu-satunya yang perlu diinstal |
| Ambxst | ya | ditolak: tidak ada repository |
| Caelestia Shell | ya | ditolak: tidak ada repository |

Caelestia dan Ambxst **tidak punya `skel/`** dan itu disengaja: keduanya tidak
ada di repository mana pun, jadi dialog menolak mereka di `Not installed` —sebelum backup diambil, bukan sesudahnya. Kalau skel-less diizinkan hanya
pada shell yang memang tidak bisa diinstal, itu yang diuji
`a_skel_is_missing_only_where_the_shell_cannot_be_installed`. Niri melanggar
aturan itu karena punya shell yang bisa diinstal, dan itu sebabnya ia dihapus
bukan dibiarkan dengan skel kosong.

## Panel log

- **Tersembunyi saat idle.** Tidak pernah menampilkan versi aplikasi atau
  daftar shell; itu isi log aplikasi, bukan panel UI.
- Panel dibungkus `gtk::Revealer` dengan `reveal_child = false` sejak awal.
- Hanya tiga pemanggil yang boleh membukanya: `log::step()`, `log::failed()`,
  `log::succeeded()`. Semua penolakan guard dan "no-op" masuk ke log aplikasi
  lewat `crate::logging`, jadi panel tidak pernah menyala karena user sekadar
  menjelajah.
- Sukses → langsung tersembunyi lagi.
- Gagal → **tetap tampil sampai ditutup** user lewat tombol `✕` di header,
  sehingga alasan gagal tidak hilang sendiri.
- Tinggi tetap 100px, melekat di bawah.
- Header berisi judul `Progress` dan tombol tutup.
- `gtk::ScrolledWindow` dengan policy horizontal `Never`, vertikal
  `Automatic`.
- Font monospace, read-only, tanpa cursor.
- Ditulis dengan `buffer.insert` di `end_iter`, satu baris per panggilan.
- `log::panel()` mengembalikan **`Revealer`**. Versi lama mengembalikan
  `ScrolledWindow` di dalam frame sehingga styling `.log-frame` tidak pernah
  terpakai, dan versi sebelum itu selalu terlihat karena tidak ada cara
  menyembunyikannya.

## Palet

Nilai di bawah diambil dari `src/style.css`, bukan dari wireframe lama. Palet
sesungguhnya ungu gelap, bukan biru.

| Token | Nilai |
|---|---|
| Background | `#14131a` |
| Tab aktif | `#4a4458` |
| Border kartu | `alpha(white, 0.16)` |
| Border kartu terpilih | `#89b4fa` |
| Teks nama | `#cfc9d6` |
| Teks redup / `section-title` | `#8b8494` |
| Nonaktif | `#6b6577` |
| Sukses / `running` | `#a6e3a1` |
| Galat / `failed` | `#f38ba8` |
| Peringatan / guard | `#f9e2af` |

Semua warna lewat class CSS di `src/style.css`.

## Catatan

- Gambar wallpaper blur yang ada di wireframe asli **tidak dipakai**; window
  sekarang warna solid. Kalau wallpaper ditambahkan nanti, tinggi log tetap
  100px dan grid tetap boleh scroll.
- Placeholder `Logo DE` di wireframe lama sudah diganti logo shell sungguhan.
- Wireframe lama menyebut 8 kartu; yang benar 4 kartu per compositor, dan
  jumlah itu boleh bertambah tanpa mengubah layout.
