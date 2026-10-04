# Keamanan Konfigurasi User

Bagian ini mengikat. Menyentuh kode backend berarti menerima semua aturan di
sini. Kalau suatu fitur tidak bisa dibuat dengan aturan ini, fiturnya yang
dipertahankan, bukan aturannya yang dilonggarkan.

## Yang tidak boleh pernah terjadi

  1. **Batasi perubahan hanya pada konfigurasi yang berkaitan dengan shell.** Picker hanya boleh mengelola file/folder yang jelas berkaitan dengan shell target; aplikasi umum tidak boleh disentuh. Semua perubahan wajib dibackup, tanpa `rm`/`mv` tanpa backup, dan tidak boleh truncate.
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
image, dan itu urusan **"RakuOS system management"**, bukan aplikasi ini.

Jadi saat user berada di Hyprland dan mengklik tab Niri, semua aksi di tab itu
harus **ditolak**, bukan dijalankan lalu gagal setengah jalan.

### Guard tidak punya arah

Penolakan ini **simetris**. Kalau user nanti berada di Niri, tab Hyprland yang
menjadi yang ditolak, dengan alasannya terbalik: "hyprland is not the running
compositor — niri is". Tidak ada kode yang menyebut Hyprland sebagai pihak yang
berwenang.

Aturannya satu perbandingan: `detection.current != compositor.id()`. Karena itu
menambah compositor ketiga tidak perlu menyentuh logika guard sama sekali.

App juga membuka tab compositor yang sedang jalan, bukan tab pertama. Kalau tidak,
user Niri akan dibuka ke halaman Hyprland yang ditolak, dengan banner kuning dan
tanpa petunjuk bahwa tab sebelahnya yang bisa dipakai.

Kedua arah ini diuji (`the_guard_works_in_both_directions`,
`the_running_compositor_is_the_only_one_that_offers_a_switch`,
`the_app_opens_on_the_running_compositors_tab`), jadi kalau suatu saat guard
diubah jadi satu arah, testnya gagal.

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
niri is not the running compositor
Switching compositor means switching image — use "RakuOS system management".
```

## Packages: `rpm -q` tidak bisa dipercaya

RakuOS jalan dalam mode *split*: base image yang read-only, plus overlay tempat
semua instalasi yang dibuat setelah image itu. `rum` menyimpan keduanya
terpisah, dan memegangnya tetap pisah adalah disengaja:

```
$ rum paths
mode: split (RakuOS overlay)
base snapshot rpmdb: /usr/share/rpm
overlay rpmdb:       /var/lib/rakuos/rum-rpmdb
```

```
$ rum -qa | wc -l                                   # 1895
$ rpm --dbpath /usr/share/rpm -qa | wc -l          # 1895
$ rpm --dbpath /var/lib/rakuos/rum-rpmdb -qa | wc -l   # 248
```

`rpm -q` membaca base saja. Dari 248 paket overlay, 243 di antaranya tidak
ditemukan `rpm -q`. Konsekuensinya kalau backend memakainya: `rum install dms`
berhasil, lalu picker tetap menampilkan "not installed" dan preflight menolak
switch ke shell yang barusan terpasang.

Jawaban yang benar:

```
$ rum origin noctalia     ->  noctalia: Base
$ rum origin firefox     ->  firefox: Overlay
$ rum origin dms          ->  dms: NotInstalled
```

Jebakan kedua: `rum info <paket>` keluar dengan status 0 bahkan kalau paketnya
tidak ada. "Tidak ada" hanya muncul di teks output:

```
$ rum info caelestia
No package named 'caelestia' found.        # exit=0
```

Jadi exit code tidak boleh dipakai sebagai pemeriksaan ketersediaan. Yang dipakai
`rum origin`, atau `rum repoquery`.

### Terpasang, dan masuk overlay, itu dua hal berbeda

| Pertanyaan | Sumber jawaban |
|---|---|
| Apakah shell-nya terpasang? | `rum origin` -> Base / Overlay / NotInstalled |
| Shell mana yang tercatat di overlay? | `/var/lib/rakuos/packages.list` dan `/var/lib/rakuos/packages-rpm.list` |

Dua file terakhir itu yang dibaca tab **Overlay** di "RakuOS system management",
jadi picker membaca file yang sama supaya tidak pernah bertentangan dengan
yang ditampilkan sistem. Bedanya: `packages.list` hanya memuat paket yang
memang diminta user, sedangkan rpmdb overlay ikut memuat seluruh dependency
yang ikut terpasang.

Konsekuensi yang sudah terlihat sekarang: `Noctalia` ada di base image,
`dms` ada di repo `rakuos` tapi belum terpasang, dan `caelestia` maupun `ambxst`
tidak ada di repo mana pun sehingga belum mungkin terpasang sama sekali.

## Wajib: backup sebelum switch

Urutan satu-satunya yang boleh dipakai:

```
 1. refuse    : guard compositor, lokasi config shell target diketahui, dan
                tidak ada shell asing yang sedang hidup. Shell target yang belum
                terpasang BOLEH diteruskan selama ia punya rencana install, karena
                backup selalu mendahului install
 2. preflight : environment + start mechanism siap (binary, unit systemd, atau
                file autostart benar-benar ada)
 3. BACKUP    : delapan path di bawah, disalin ke
                backups/<waktu>-<compositor>-<shell>/ lalu DIBACA ULANG untuk
                dicocokkan dengan manifest
                                            <- gagal di sini, switch dibatalkan
4. catat state "sedang berganti"           -> supaya sesi bisa dipulihkan
  5. INSTALL   : satu privileged transaction per tahap, pkexec rum install -y
                 <paket>, hanya untuk paket yang memang belum ada
                                             <- gagal di sini, switch dibatalkan
  6. terapkan skel : config milik shell diganti penuh; hypr hanya lewat shim
  7. stop      : shell lama diberi SIGTERM -> ditunggu -> batas waktu -> kill -9
                 hanya sebagai upaya terakhir
  8. start     : shell baru, sesuai StartMechanism-nya
  9. verify    : shell baru benar-benar hidup dalam batas waktu
 10. state     : tulis active/previous
 11. ROLLBACK  : kalau langkah 8/9 gagal, start lagi shell lama dan kembalikan
                 config dari backup
```

**Tidak ada langkah "cek dependency" terpisah, dan itu disengaja.** `rum install`
memang sudah menyelesaikan transaksi itu sendiri: paket yang sudah ada dilewati,
yang kurang ditarik, dan Provide yang salah tidak akan terpilih. Pemeriksaan
terpisah hanya mengulang pekerjaan itu, menambah detik ke setiap klik, dan
membaca metadata yang bisa sudah basi saat install berjalan. Yang unsafe bukan
"solvernya memilih" — unsafe kalau kita **tidak menentukan** pilihannya. Maka
nama paket ditulis langsung di command line, dan itu dibaca pada saat yang
benar:

| paket | ditulis karena |
| --- | --- |
| `quickshell-git` | `dms` meminta `(quickshell or quickshell-git)`; repo juga punya `noctalia-qs` yang menyediakan kapabilitas sama. Itu build Noctalia lama, hanya dibutuhkan paket `noctalia-legacy` yang sudah mati. |
| `dms-cli` | hard requirement yang versinya dikunci `= 1.6.2-1.fc44` |
| `dankcalendar-git` | **tidak menjadi dependency RPM DMS sama sekali**, tapi `quickshell/Modules/DankDash` memakainya. Tanpa itu widget kalender rusak, dan tidak ada metadata yang akan memberi tahu. |

Install dipecah jadi dua tahap, `quickshell-git` lebih dulu sendirian. Kalau
tahap itu gagal, tahap kedua tidak dijalankan, jadi tidak ada satu pun paket DMS
di mesin. Menggabungkan keduanya berarti kegagalan runtime ikut menjatuhkan
paket shell tanpa keuntungan apa pun.


Backup selalu **lebih dulu**, sebelum satu pun langkah yang menulis. Backup
hanya membaca, jadi tidak ada ruginya kalau ternyata langkah berikutnya batal.
Install adalah langkah pertama yang menyentuh sistem, jadi ia diletakkan
setelah semua langkah yang masih boleh menolak.

### Yang di-backup

Delapan path, relatif terhadap `$HOME`:

```
.config/gtk-3.0            bersama     milik sesi, bukan milik shell
.config/gtk-4.0            bersama
.config/hypr               bersama     hanya lewat fragment, tidak pernah utuh
.config/qt5ct              bersama
.config/qt6ct              bersama
.config/noctalia           shell       keluar
.local/state/noctalia      shell       keluar
```

`~/.local/state/noctalia` ikut karena sebagian besar setup Noctalia ada di
sana: 389 file, bukan config ringan. Path yang tidak ada dicatat sebagai
`absent`, bukan dianggap gagal, supaya saat restore "tidak pernah ada" bisa
dibedakan dari "hilang".

### Apa yang boleh ditimpa, dan apa yang tidak

- **Config milik shell itu sendiri boleh diganti penuh** setelah backup
  terverifikasi. `~/.config/<shell>` dan `~/.local/state/<shell>` milik shell
  tersebut, jadi menggantinya bukan merusak milik user.
- **`~/.config/hypr` tidak pernah diganti penuh.** Isinya 45 file dan lebih dari
  seribu baris config yang bukan tentang shell sama sekali. Picker hanya
  menulis fragment miliknya sendiri di sana.
- **gtk/qt ikut diganti** karena menentukan tampilan seluruh program, bukan
  cuma shell. Efeknya lebih luas daripada shell, dan justru karena itu semua
  ikut di-backup dan tetap dibiarkan ada file aslinya untuk restore manual.

Aturan penting dari urutan itu:

- **Langkah 3 boleh menggagalkan switch.** Lebih baik batal daripada merusak.
- **Kalau backup gagal, switch tidak jalan.** Ini bukan peringatan, ini syarat.
- **Backup yang tidak bisa dibaca ulang dianggap tidak ada.** Switch tidak
  pernah lanjut dengan anggapan ada jalan pulang.
- **Tidak ada penghapusan backup secara otomatis.** Pembersihan hanya kalau
  user sendiri yang memintanya, dan jumlah backup ditampilkan dulu.

## Boleh dan tidak boleh dilakukan picker

Batasnya bukan "tidak boleh menyentuh apa pun", tapi "boleh hanya setelah
syaratnya terpenuhi". Yang tidak punya syarat ada di tabel kedua.

| Boleh | Syaratnya |
|---|---|
| Memasang paket yang belum ada, lewat `pkexec` | Bukan `rum` yang dipanggil langsung. `pkexec` yang minta otorisasi ke polkit agent sistem, jadi password tidak pernah masuk ke proses picker. Kegagalan instalasi membatalkan switch, bukan melanjutkan setengah jalan. |
| Mengganti config milik shell itu sendiri | Hanya setelah backup tersimpan dan bisa dibaca ulang. Ganti, bukan menimpa buta: config lama ada di backup dan dikembalikan lewat copy-paste manual. |
| Mematikan autostart shell sebelumnya | Supaya tidak ada dua shell hidup. Paketnya dibiarkan supaya switch-back selalu mungkin. |

| Tidak | Alasannya |
|---|---|
| Uninstall di dalam alur switch | Shell yang sudah di-uninstall tidak bisa di-switch-back. Uninstall jadi tindakan terpisah dengan tombol dan konfirmasi sendiri, bukan efek samping dari switch. |
| Mengganti config compositor | `~/.config/hypr/` milik user dan tidak pernah ditulis ulang. Picker hanya menulis ke file miliknya sendiri. |
| `rm -rf` config tanpa backup yang terbukti tersimpan | Backup yang gagal membatalkan switch. Tidak ada kondisi di mana config hilang dan switch tetap jalan. |

### Tombol uninstall

Terpisah dari switch, di card masing-masing shell, dengan konfirmasi sendiri.
Salah klik berarti kehilangan shell, dan itu tidak bisa di-rollback.

Untuk Noctalia tombolnya mati. Dia ada di base image yang read-only, jadi
`rum remove` akan menolaknya; tombol yang selalu gagal lebih buruk daripada
tidak ada.

`rum remove` tidak uninstall dependency yang masih dipakai paket lain. Sisa
begitu adalah kandidat `rum autoremove`, dan itu ditawarkan sebagai pilihan
tersendiri, bukan dijalankan diam-diam.

## Skel: mana yang diganti, mana yang dipecah

Bentuk skel tidak seragam, dan tidak bisa seragam karena ada dua pemilik
config yang berbeda:

| Dipasang penuh | Dipasang sebagai fragment |
|---|---|
| `~/.config/<shell>/` | `~/.config/hypr/` |
| `~/.config/gtk-3.0`, `gtk-4.0`, `qt5ct`, `qt6ct` | — |

Alasannya, `~/.config/hypr/` adalah milik user. Jejak sebuah shell di sana
tersebar di beberapa file sekaligus — untuk Noctalia ada di `hyprland.lua`,
`config/autostart.lua`, `config/binds.lua`, `config/lid.lua`,
`config/windowrules.lua`, dan `scripts/lock-and-suspend.sh`. Menyalin direktori
ini secara utuh tidak akan menghapus "jejak noctalia"; ia akan menghapus seluruh
konfigurasi user, termasuk yang tidak ada hubungannya dengan shell.

Jadi picker hanya menulis file milik sendiri, dan kalau autostart perlu diubah,
perubahannya dilakukan satu kali lewat file shim yang membaca `state.toml`. Sesudah
itu, ganti shell tidak pernah menyentuh Lua milik user lagi.

`uwsm` tidak ikut berperan, dan tidak pernah ada. Rantainya `start-hyprland` ->
`Hyprland` -> `noctalia`: UWSM menjalankan kompositor lalu turun, tidak ada
proses uwsm di rantai itu. Yang menjalankan shell adalah baris
`hl.exec_cmd("noctalia")` di `config/autostart.lua`. `~/.config/uwsm`
lagi tidak per-desktop sama sekali — bagian per-desktopnya ada di sistem,
`/usr/share/uwsm/plugins/<desktop>.sh`, yang tidak pernah kita sentuh; bagian
user-nya satu skrip `env` global yang dipakai bersama semua sesi. Jadi fragmen
uwsm akan identik untuk setiap kompositor, yaitu bukan urusan ganti shell,
cuma risiko merusak file milik desktop lain.

Bentuk direktori:

```
src/compositor/hyprland/shells/
  noctalia/
    mod.rs           -> apa yang ditulis switch untuk Noctalia di Hyprland
    skel/            -> file yang dipasang switch
  dms/
    mod.rs
    skel/
```

Skel ada di dalam folder shell-nya, bukan di folder `skel/` terpisah, supaya
satu shell di satu compositor utuh di satu tempat: `mod.rs` menyatakan ke mana
file-nya ditulis dan mana yang perlu diselamatkan, `skel/` berisi file aslinya.
Tidak perlu mencari dua tempat untuk_EQ satu shell.

Tidak semua shell punya default config untuk disalin. Noctalia tidak mengirim
satu pun, dia membuat `~/.config/noctalia/config.toml` sendiri saat pertama
jalan. Karena itu tiap shell perlu ditandai: salin dari skel, atau biarkan
shell yang membuat default-nya sendiri.

## Apa yang wajib diselamatkan, dan siapa yang mengatakannya

Switch menulis tiga kelompok path, dan ketiganya harus punya backup:

| Yang ditulis switch | Dizukan dari |
|---|---|
| config compositor | `Compositor::config_dir` |
| `config_dir` + `state_dir` shell | `Port`, otomatis |
| `skel_extra_paths` | `Port::skel_extra_paths` |
| `backup_extra_paths` | `Port::backup_extra_paths`, per shell |

Dua kelompok terakhir yang paling mudah terlewat. `skel_extra_paths` adalah daftar file yang
ditimpa, jadi setiap isinya wajib ada di backup. Dulu daftar backup diturunkan
dari `config_dir` dan `state_dir` saja, sehingga tiga file ditulis tapi tidak
diselamatkan: launcher `.desktop` milik Noctalia, dan direktori state DMS. Switch
bisa menghapus salinan user tanpa ada jalan kembali — persis hasil yang aturan 2
minta untuk dicegah.

Sekarang tiap shell mendeklarasikan `backup_extra_paths` sendiri, dan test
`every_installed_path_is_backed_up` mengunci hubungan itu: menambahkan path ke
`skel_extra_paths` tanpa menambahkannya ke backup akan menggagalkan test, bukan
mengirim switch yang tidak bisa dibatalkan.

Bedanya antara "dipasang" dan "perlu disimpan" sengaja tidak disamakan. DMS
memasang `dev.noctalia.Noctalia.desktop` untuk menyembunyikan launcher Noctalia,
tetapi file itu milik Noctalia — jadi yang menyimpannya adalah shell yang
memilikinya, bukan yang menutupnya.

## Me-restart compositor bukan cara ganti shell

Compositor tidak mengenal shell. `hyprctl reload` hanya membaca ulang
konfigurasi; tidak menjalankan shell apa pun. Jadi "reload lalu otomatis
berganti" bukan mekanisme yang benar — digantungkan begitu saja, tidak akan
pernah terjadi.

Shell harus dihentikan dan dijalankan ulang oleh picker sendiri, sementara
compositor dibiarkan terus berjalan. Ini aturan 2 di atas, dan tidak ada
pengecualian untuk hal ini.

## Lokasi backup

```
# $XDG_DATA_HOME/backups/rakuos-shell-picker/, yaitu ~/.local/share/... di sini
# BUKAN $XDG_STATE_HOME. State adalah tempat untuk bendahara yang tidak penting
# dan aman dihapus antar-run; backup justru bendahara yang paling perlu selamat,
# jadi diletakkan di share. Kalau tidak, satu "rm -rf ~/.local/state/*" untuk
# bersihin cache akan ikut menghapus seluruh recovery copy.
```
~/.local/share/backups/rakuos-shell-picker/
  20261003-022126-hyprland-noctalia/
    manifest.toml          isi tiap path, jumlah file, ukuran, symlink
    home/                  salinan, mirroring path asli relatif terhadap $HOME
  20261002-190112-hyprland-dms/
    ...
```

Backup bersifat **tambahan**, bukan pengganti: config asli tetap di tempat
asalnya. Menyalin, bukan memindahkan.

## Isi `manifest.toml`

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
- [ ] Test `manifest.toml`: isi tiap path, jumlah file, ukuran, symlink.
- [ ] Test yang membuktikan config asli masih utuh setelah backup.
- [ ] Test bahwa backup yang rusak ditolak: hapus satu file dari salinan, lalu
      `verify()` harus menolak dan menyebut file yang hilang.
- [ ] Test rollback dengan target yang dijamin gagal start.
- [ ] Deteksi "shell lain yang jalan di luar daftar kita" sebelum switch, supaya
      picker tidak diam-diam menyalakan shell kedua.
- [ ] Batas waktu untuk stop/start, supaya tidak menggantung selamanya.
- [ ] Test rotasi log gagal: lebih dari 10 file, yang tertua dipangkas, backup
      konfigurasi **tidak** ikut terpangkas.
