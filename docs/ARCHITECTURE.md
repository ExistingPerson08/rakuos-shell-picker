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

src/compositor/             satu folder per compositor
  mod.rs                    trait Compositor + registry tab
  hyprland/mod.rs           alat khusus Hyprland (hyprctl, window rule)
  hyprland/shells/          registri shell untuk Hyprland
    mod.rs                  urutan kartu
    noctalia.rs             satu file per shell
    dms.rs
    caelestia.rs
    ambxst.rs
  niri/mod.rs               alat khusus Niri, shells masih kosong

src/shell.rs                tipe Shell + cara memeriksa satu shell
src/session.rs              state active/previous di XDG
```

## Arah dependensi

```
app ──▶ ui ──▶ compositor ──▶ shell
                    │
                    └───▶ shell (untuk registry)
session ──▶ shell
```

- `ui/` tidak pernah mengimpor `compositor::hyprland::shells::*` — itu hanya
  boleh muncul di `compositor/mod.rs`.
- `shell.rs` **tidak** tahu shell apa yang ada. Daftar shell milik tiap
  compositor. `shell.rs` hanya berisi tipe dan cara menginspeksinya.
- `compositor/niri/` tidak boleh mengimpor dari `compositor/hyprland/`.

## Menambah shell

Cuma dua file:

```rust
// src/compositor/hyprland/shells/nama_baru.rs
use crate::shell::{Shell, StartMechanism};

pub static SHELL: Shell = Shell {
    id: "nama_baru",
    display_name: "Nama Baru",
    upstream: "org/nama-baru",
    package: None,
    binary: "nama-baru",
    start_argv: &["nama-baru"],
    config_dir: Some(".config/nama-baru"),
    systemd_user_unit: None,
    mechanism: StartMechanism::DirectProcess,
    health_socket: None,
    logo: Some("nama-baru.svg"),
};
```

```rust
// src/compositor/hyprland/shells/mod.rs
pub mod nama_baru;
// ...
pub static ALL: &[Shell] = &[..., nama_baru::SHELL];
```

Selesai. Tab, grid, kartu, dan log ikut menyesuaikan sendiri. `ui/` tidak
disentuh. Test `every_registered_shell_has_its_logo_on_disk` akan gagal kalau
SVG-nya lupa ditaruh di `assets/`.

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

Compositor dengan daftar kosong otomatis dapat halaman "Coming soon".

## Konvensi nama

| Hal | Nilai |
|---|---|
| Id compositor | huruf kecil, nama compositor asli (`hyprland`, `niri`) |
| Id shell | huruf kecil, nama upstream (`noctalia`, `dms`) |
| Konstanta layout | di file `ui/` yang memakainya, bukan di root |
| File statis | `UPPER_SNAKE` untuk `Shell`, tipe untuk compositor |

## Test

Ada unit test tanpa display server:

```bash
cargo test
```

Yang dijaga: id compositor unik, id shell unik lintas compositor, setiap shell
punya logo yang benar-benar ada di disk, urutan kartu Hyprland, Niri masih
kosong, dan roundtrip state session.

Yang belum ada (wajib ditambahkan saat menambah backend): test untuk logika
backup dan switch.
