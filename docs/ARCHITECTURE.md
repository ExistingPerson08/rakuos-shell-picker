# Arsitektur

## Peta modul

```
src/main.rs                 hanya memanggil app::run()
src/app.rs                  aplikasi GTK/libadwaita, window, CSS
src/lib.rs                  daftar modul publik + test registry

src/ui/                     widget — tidak tahu shell, tidak tahu compositor
  mod.rs                    menyusun halaman, memegang Selection
  tabbar.rs                 satu tab per compositor
  grid.rs                   grid kartu untuk satu compositor + placeholder
  card.rs                   satu kartu shell
  logo.rs                   pemuatan SVG + kotak logo
  log.rs                    panel log

src/shells/                 satu file per shell — apa shell itu, di compositor mana pun
  mod.rs                    daftar semua identity (untuk test)
  noctalia.rs               IDENTITY: id, nama, upstream, paket, install plan, logo
  dms.rs
  caelestia.rs
  ambxst.rs

src/compositor/             satu folder per compositor
  mod.rs                    trait Compositor + registry tab
  hyprland/
    mod.rs                  alat khusus Hyprland (hyprctl, window rule)
    shells/                 satu folder per shell — shell itu DI HYPRLAND
      mod.rs                urutan kartu
      noctalia/
        mod.rs              PORT: ke mana ditulis, apa yang diselamatkan
        skel/               file yang dipasang switch (.config/hypr, gtk, launcher)
      dms/
        mod.rs
        skel/
      caelestia/mod.rs      belum ada skel — switch ke Caelestia ditolak
      ambxst/mod.rs         belum ada skel — config-nya belum diketahui
  niri/
    mod.rs
    shells/
      mod.rs
      noctalia/mod.rs       belum ada skel — switch ke Niri ditolak, SAFETY.md
      dms/mod.rs
      ambxst/mod.rs         (caelestia tidak ada: upstream hanya kirim Hyprland)

src/shell.rs                tipe Identity + Port + Shell, dan cara memeriksa satu shell
src/session.rs              state active/previous di XDG
```

## Kenapa shell dipecah dua

Dulu setiap `(compositor, shell)` punya satu `Shell` penuh, jadi data yang
benar-benar sama ditulis dua kali: `upstream`, `package`, `install_plan`, dan
`companion_systemd_user_units` ulang untuk Noctalia di Hyprland dan di Niri.
Tidak ada yang menolak compile kalau salah satu tidak sengaja diubah — dan
akibatnya bukan error kompilasi, melainkan satu tab memeriksa paket yang tab
lain memasang.

Sekarang `Identity` memegang yang benar-benar milik shell, dan `Port` hanya
yang berubah menurut compositor:

| Field | `Identity` | `Port` |
|---|---|
| `id`, `display_name`, `upstream` | `Identity` | — |
| `package`, `start_argv`, `logo` | `Identity` | — |
| `install_plan`, `companion_systemd_user_units` | `Identity` | — |
| `config_dir`, `state_dir` | — | `Port` |
| `systemd_user_unit`, `mechanism` | — | `Port` |
| `skel_extra_paths`, `health_socket` | — | `Port` |

`Port` sengaja tidak punya field yang bisa berduplikasi, jadi menyalinnya ke
compositor lain tidak bisa gagal diam-diam. `Shell::join` adalah `const fn`,
jadi `Shell` yang dirakit tetap bisa dipakai di `static` tanpa biaya runtime.

Yang tetap di `switch.rs`, bukan diulang per shell: config bersama yang selalu
disimpan backup-nya, dan urutan langkah switch.

## Arah dependensi

```
app ──▶ ui ──▶ compositor ──▶ shell ◀── shells
                     │
                     └───▶ shell (untuk registry)
session ──▶ shell
```

- `ui/` tidak pernah mengimpor `compositor::hyprland::shells::*` — itu hanya
  boleh muncul di `compositor/mod.rs`.
- `shell.rs` **tidak** tahu shell apa yang ada. Daftar shell milik tiap
  compositor. `shell.rs` hanya berisi tipe dan cara menginspeksinya.
- `compositor/niri/` tidak boleh mengimpor dari `compositor/hyprland/`.
- `compositor/*/shells/*.rs` boleh mengimpor `crate::shells::*`, dan tidak
  boleh mengimpor `crate::shells` milik compositor lain.

## Test yang menjaga pemisahan ini

Di `src/lib.rs`, `mod registry_tests`:

- `one_identity_serves_every_compositor_offering_that_shell` — membandingkan
  setiap field yang dimiliki `Identity` dengan yang muncul di tiap compositor.
  Karena `Port` tidak punya field itu, test ini hanya bisa gagal kalau ada yang
  menambahkannya kembali ke `Port`.
- `every_identity_is_offered_by_at_least_one_compositor` — menangkap file
  identity yang tidak terdaftar di compositor mana pun: ia compile, ia lolos
  semua test lain, dan ia tidak ada di kartu mana pun.
- `every_port_stays_inside_home` — semua path relatif, karena semuanya
  digabung ke `$HOME` lalu ditulis.
- `every_declared_skel_extra_path_really_is_in_the_skel` — `apply::plan`
  memperlakukan path yang hilang sebagai sekadar melewati, jadi path yang sudah
  di-rename dulunya hilang diam-diam.
- `every_installed_path_is_backed_up` — setiap file yang ditulis switch dari skel
  wajib punya backup. Ini yang menahan penambahan `skel_extra_paths` tanpa
  Declaration backup yang cocok.

## Menambah shell

Dua folder, satu di global dan satu per compositor.

```rust
// src/shells/nama_baru.rs — yang benar-benar milik shell, di mana pun
use crate::shell::{Identity, InstallPlan, Shell};

pub const IDENTITY: Identity = Identity {
    id: "nama_baru",
    display_name: "Nama Baru",
    upstream: "org/nama-baru",
    package: None,
    binary: "nama-baru",
    start_argv: &["nama-baru"],
    companion_systemd_user_units: &[],
    logo: Some("nama-baru.svg"),
    install_plan: None,
};
```

Lalu satu folder per compositor yang menawarkannya, isinya lengkap dari install
sampai backup:

```
src/compositor/hyprland/shells/nama_baru/
  mod.rs      PORT: ke mana ditulis, apa yang diselamatkan
  skel/       file yang dipasang switch
```

```rust
// src/compositor/hyprland/shells/nama_baru/mod.rs
use crate::shell::{Port, Shell, StartMechanism};
use crate::shells::nama_baru::IDENTITY;

const PORT: Port = Port {
    config_dir: Some(".config/nama-baru"),
    state_dir: None,
    systemd_user_unit: None,
    mechanism: StartMechanism::DirectProcess,
    skel_extra_paths: &[],
    backup_extra_paths: &[],
    health_socket: None,
};

pub static SHELL: Shell = Shell::join(IDENTITY, PORT);
```

Daftarkan di `compositor/hyprland/shells/mod.rs`:

```rust
pub mod nama_baru;
// ...
pub static ALL: &[Shell] = &[..., nama_baru::SHELL];
```

Ulangi folder itu untuk setiap compositor lain yang menawarkannya.
`niri/shells/nama_baru/mod.rs` akan berbeda di `mechanism` atau
`systemd_user_unit` kalau memang begitu, dan identik kalau memang sama.

Dua field yang sering tertukar:

- `skel_extra_paths` — file yang switch **timpa**. Tambah di sini berarti
  menimpa file milik user, jadi harus ada di `backup_extra_paths` atau
  `SHARED_PATHS`. `every_installed_path_is_backed_up` akan menggagalkan kalau
  tidak.
- `backup_extra_paths` — file milik shell ini yang switch hancurkan tanpa
  tercatat di `config_dir`/`state_dir`. Contohnya milik DMS: direktori state-nya
  sampai lewat skel, bukan lewat `state_dir`.

Test `every_registered_shell_has_its_logo_on_disk` gagal kalau SVG-nya lupa
ditaruh di `assets/`.

## Menambah compositor

```rust
// src/compositor/sway/mod.rs
use crate::compositor::Compositor;
use crate::shell::Shell;

pub struct Sway;
impl Compositor for Sway {
    fn id(&self) -> &'static str { "sway" }
    fn label(&self) -> &'static str { "sway" }
    fn shells(&self) -> &'static [Shell] { &[] }  // placeholder dulu
}
```

Lalu daftarkan di `compositor/mod.rs`:

```rust
pub mod sway;
pub static SWAY: sway::Sway = sway::Sway;

pub fn all() -> Vec<&'static dyn Compositor> {
    vec![&HYPRLAND, &NIRI, &SWAY]
}
```

Compositor dengan daftar kosong otomatis dapat halaman "Coming soon". Setelah
daftarnya tidak kosong, tambahkan satu folder per shell di `sway/shells/<shell>/`
— `mod.rs` berisi `Port`-nya sendiri, `skel/` berisi file yang dipasang —
dengan `Identity` yang sudah ada di `src/shells/` tidak disalin.

Perhatikan bahwa Niri sudah punya tiga kartu di tabnya, tapi folder
`niri/shells/*/skel/` belum ada satu pun. Switch ke sana ditolak `apply::plan`
dengan pesan bahwa config compositor-nya belum ada. Itu disengaja: menampilkan
apa yang akan didapat lebih jujur daripada diam-diam mengganti sebagian
konfigurasi lalu meninggalkan sisanya milik shell lama.

## Konvensi nama

| Hal | Nilai |
|---|---|
| Id compositor | huruf kecil, nama compositor asli (`hyprland`, `niri`) |
| Id shell | huruf kecil, nama upstream (`noctalia`, `dms`) |
| Konstanta layout | di file `ui/` yang memakainya, bukan di root |
| File statis | `UPPER_SNAKE` untuk `Shell`, `IDENTITY` untuk `Identity`, `UPPER_SNAKE` untuk `Port` |
| Letak skel | `compositor/<c>/shells/<shell>/skel/`, di dalam folder shell |

## Test

Ada unit test tanpa display server:

```bash
cargo test
```

Yang dijaga: id compositor unik, id shell unik lintas compositor, setiap shell
punya logo yang benar-benar ada di disk, urutan kartu tiap compositor, setiap
identity dipakai oleh minimal satu compositor, field `Identity` tidak pernah
berbeda antar compositor, semua path tetap di dalam `$HOME`, setiap
`skel_extra_paths` benar-benar ada di skel compositor itu, roundtrip state
session, dan collision nama backup.

Test yang butuh display server ditandai `#[ignore]`:

```bash
cargo test -- --ignored
```

Sebelum commit, `cargo fmt --check`, `cargo clippy --all-targets`, dan
`cargo test` harus bersih. Suite ini pernah punya test backup yang gagal 13 dari
15 kali karena nama directory berbasis milidetik, jadi kalau sebuah test gagal
dengan intermittently, cek dulu apakah itu memang flaky sebelum memperbaikinya.
