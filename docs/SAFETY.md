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

## State file

`~/.local/state/rakuos/shell-picker/state.toml`

- Ditulis secara atomik: tulis file sementara di direktori yang sama, lalu
  `rename`. `fs::write` langsung ke `state.toml` bisa meninggalkan file
  setengah tertulis kalau supply daya hilang di tengah jalan.
- Isinya hanya id shell & compositor. **Tidak pernah** berisi isi konfigurasi.
- Rusak tidak boleh jadi kegagalan fatal: state yang tidak terbaca diperlakukan
  sebagai state kosong, dengan warning di log.

## Yang perlu ditambahkan sebelum backend hidup

- [ ] Test untuk `metadata.toml`: timestamp, `outcome`, nama shell.
- [ ] Test yang membuktikan config asli masih utuh setelah backup.
- [ ] Test rollback dengan target yang dijamin gagal start.
- [ ] Deteksi "shell lain yang jalan di luar daftar kita" sebelum switch, supaya
      picker tidak diam-diam menyalakan shell kedua.
- [ ] Batas waktu untuk stop/start, supaya tidak menggantung selamanya.
