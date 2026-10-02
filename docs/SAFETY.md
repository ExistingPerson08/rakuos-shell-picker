# Keamanan Konfigurasi User

Bagian ini mengikat. Menyentuh kode backend berarti menerima semua aturan di
sini. Kalau suatu fitur tidak bisa dibuat dengan aturan ini, fiturnya yang
dipertahankan, bukan aturannya yang dilonggarkan.

## Yang tidak boleh pernah terjadi

1. **Tidak pernah menghapus atau menimpa konfigurasi user.** Tidak `rm`, tidak
   `mv` tanpa backup, tidak truncate. Config yang diedit manual sering tidak
   bisa direkonstruksi.
2. **Tidak pernah me-restart atau mematikan compositor.** Hyprland/Niri yang
   sedang menjalankan sesi user tetap jalan selama picker hidup. Kalau picker
   mati, compositor harus tetap hidup dan shell harus tetap bisa jalan ulang.
3. **Tidak menyentuh compositor lain.** Saat switch di tab Niri, konfigurasi
   Hyprland tidak boleh dibaca untuk ditulis, dan sebaliknya.
4. **Tidak mengarang lokasi konfigurasi.** Kalau `config_dir` untuk sebuah shell
   belum diketahui, backend itu belum boleh jalan — bukan menebak
   `~/.config/<nama>`.
5. **Tidak menjalankan `kill -9` sebagai cara normal.** Shell harus diberi
   kesempatan berhenti rapi; `kill -9` hanya boleh jadi jalan keluar terakhir
   setelah timeout.
6. **Tidak pernah menyalakan shell milik compositor lain.** Lihat
   [Deteksi dan Guard](#deteksi-dan-guard) di bawah.
7. **Tidak pernah menebak compositor yang sedang jalan.** Kalau tidak
   terdeteksi, tidak ada switch sama sekali.

## Deteksi dan Guard

Compositor tidak berganti lewat picker. Mengganti compositor berarti mengganti
image, dan itu urusan **rakuos system management**, bukan aplikasi ini.

Jadi saat user berada di Hyprland dan mengklik tab Niri, semua aksi di tab itu
harus **ditolak**, bukan dijalankan lalu gagal setengah jalan.

### Cara menentukan compositor yang jalan

Envvar khusus compositor, yang paling bisa dipercaya:

| Compositor | Penanda | Andal |
|---|---|---|
| Hyprland | `HYPRLAND_INSTANCE_SIGNATURE` terisi | ya |
| Niri | `NIRI_SOCKET` ada dan socket-nya bisa dibuka | ya |
| Sway | `SWAYSOCK` | ya |

Ditambah pemeriksaan dasar yang wajib lolos dulu: `WAYLAND_DISPLAY` dan
`XDG_RUNTIME_DIR` terisi.

`XDG_CURRENT_DESKTOP` **tidak boleh dipakai** untuk guard — nilainya bisa
dikarang user, sehingga tidak layak jadi dasar keputusan keselamatan.

Kalau penanda khusus tidak ada, urutannya: cek `WAYLAND_DISPLAY` (namanya
kadang memuat compositor), lalu scan `/proc/*/exe`. Envvar tetap cara utama.

### Kalau tidak terdeteksi

Tidak terdeteksi berarti picker dijalankan di luar sesi Wayland yang bisa
diandalkan. Dalam keadaan ini: **tolak semua switch**, tampilkan peringatan,
jangan menebak. Menjalankan shell yang salah lebih buruk daripada tidak
menjalankan apa pun.

### Dua lapis pertahanan

| Lapis | Wajib | Tanggung jawab |
|---|---|---|
| UI | ya | Card untuk compositor yang tidak sedang jalan diberi class `disabled`, tidak bisa diklik, ada badge penjelas |
| Backend | **ya** | `preflight()` menolak dengan alasan yang jelas |

UI saja tidak cukup — guard bisa dilewati. Backend yang benar-benar menegakkan
aturan; UI cuma supaya user tidak dicoba-coba.

Pesan penolakan yang dipakai:

```
niri is not the running compositor (hyprland is).
Switching compositor means switching image — use rakuos system management.
```

## Wajib: backup sebelum switch

Urutan satu-satunya yang boleh dipakai:

```
1. cek environment  (display, Wayland, shell target benar-benar terpasang)
2. cek backend sehat (bisa start, config path ketemu)
3. BACKUP config shell yang AKTIF           <- gagal di sini, batal
4. catat state "sedang berganti"            <- supaya sesi bisa dipulihkan
5. stop shell lama dengan rapi
6. start shell baru
7. verifikasi shell baru benar-benar hidup
8. tulis state active/previous
9. kalau langkah 6/7 gagal -> ROLLBACK dari backup
```

Aturan penting dari urutan itu:

- **Langkah 3 boleh menggagalkan switch.** Lebih baik batal daripada merusak.
- **Kalau backup gagal, switch tidak jalan.** Ini bukan peringatan, ini syarat.
- **Tidak ada penghapusan backup secara otomatis.** Pembersihan hanya kalau
  user sendiri yang memintanya, dan jumlah backup ditampilkan dulu.

## Lokasi backup

```
~/.local/state/rakuos/shell-picker/backups/
  20261002-183045-hyprland-noctalia/
    metadata.toml          shell, compositor, waktu, asal, hasil
    config/                salinan ~/.config/noctalia
  20261002-190112-hyprland-dms/
    ...
```

Backup bersifat **tambahan**, bukan pengganti: config asli tetap di tempat
asalnya. Menyalin, bukan memindahkan.

## Isi `metadata.toml`

Minimum ini, supaya rollback bisa memutuskan sendiri apa yang harus dilakukan:

```toml
from_shell = "noctalia"
from_compositor = "hyprland"
to_shell = "dms"
to_compositor = "hyprland"
started_at = "2026-10-02T18:30:45Z"   # RFC 3339, UTC
outcome = "started"                    # started | failed | rolled_back
app_version = "0.1.0"
```

## Rollback

Rollback wajib kalau: proses target tidak hidup dalam batas waktu, atau target
langsung mati setelah start.

Rollback berarti: kembalikan backup ke tempat asalnya, start lagi shell lama,
tandai entri backup sebagai `rolled_back`, catat kejadian di log. Membatalkan
rollback butuh alasan yang eksplisit dari user.

## Logging saat gagal

Detail kegagalan ditulis ke file terpisah, bukan ke panel UI supaya tetap bisa
dibaca setelah app ditutup. Aturan lengkapnya ada di [LOGGING.md](LOGGING.md).

Yang dicatat: `preflight`, `backup`, `stop`, `start`, `verify`, `state`.
Yang **tidak** dicatat sebagai kegagalan: guard yang ditolak dari awal, dan
shell yang memang belum terpasang — keduanya sudah terlihat lewat badge di card.

## State file

`~/.local/state/rakuos/shell-picker/state.toml`

- Ditulis secara atomik: tulis file sementara di direktori yang sama, lalu
  `rename`. `fs::write` langsung ke `state.toml` bisa meninggalkan file
  setengah tertulis kalau supply daya hilang di tengah jalan.
- Isinya hanya id shell & compositor. **Tidak pernah** berisi isi konfigurasi.
- Rusak tidak boleh jadi kegagalan fatal: state yang tidak terbaca diperlakukan
  sebagai state kosong, dengan warning di log.

## Yang perlu ditambahkan sebelum backend hidup

- [ ] Test `detect_compositor()` untuk tiap compositor: envvar terisi, kosong,
      dan socket yang tidak bisa dibuka.
- [ ] Test penolakan guard: compositor terdeteksi A, lalu backend untuk B
      dipanggil — harus menolak, dan tidak boleh menyentuh apa pun.
- [ ] Test `metadata.toml`: timestamp, `outcome`, nama shell.
- [ ] Test yang membuktikan config asli masih utuh setelah backup.
- [ ] Test rollback dengan target yang dijamin gagal start.
- [ ] Deteksi "shell lain yang jalan di luar daftar kita" sebelum switch, supaya
      picker tidak diam-diam menyalakan shell kedua.
- [ ] Batas waktu untuk stop/start, supaya tidak menggantung selamanya.
- [ ] Test rotasi log gagal: lebih dari 10 file, yang tertua dipangkas, backup
      konfigurasi **tidak** ikut terpangkas.

