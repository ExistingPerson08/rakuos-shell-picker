# Hyprland Configuration — Quick Guide

Reload after editing: `SUPER + SHIFT + R`

> **Important for existing users.** This folder is *copied* into your home the
> first time you log in on this desktop environment. From then on Hyprland reads
> the copy in your own home, so edit the files there — changes to `/etc/skel`
> only reach accounts that have not been populated yet, and are copied without
> overwriting anything you already have.

---

## Changing monitors?

**`config/variables.lua`**

```lua
MONITOR1 = "eDP-1"
MONITOR2 = "DP-1"
```

List the outputs available on your machine:

```bash
hyprctl monitors
```

Use the value from the `NAME` column. Leave it empty (`""`) if there is no second
monitor.

These variables are read by:

- `config/monitors.lua` — the output that gets configured
- `config/workspaces.lua` — workspaces 1-5 go to `MONITOR1`, 6-9 to `MONITOR2`
- `PRIMARY_MONITOR` — where launcher, Discord and `.exe` windows are opened

---

## Changing or adding default apps?

**`config/variables.lua`**

```lua
TERMINAL     = "ghostty"
FILE_MANAGER = "nautilus"
BROWSER      = "firefox"
EDITOR       = "zeditor"
CALCULATOR   = "gnome-calculator"
```

`binds.lua` reads these variables, so this is the only file you need to touch —
no need to edit the binds themselves. Session-aware launching goes through the
`launchPrefix` (`uwsm app -- `) defined in `binds.lua`.

---

## Where are the keyboard binds?

**`config/binds.lua`** — every `SUPER + ...` binding is here. `M` stands for
`SUPER`.

Example:

```lua
hl.bind(M .. " + Return", hl.dsp.exec_cmd(launchPrefix .. TERMINAL), { description = "Terminal" })
```

Shortcuts worth knowing:

| Shortcut | Action |
|---|---|
| `SUPER + Return` | Terminal |
| `SUPER + E` / `B` / `N` | File manager / Browser / Editor |
| `SUPER + Space` | App launcher |
| `SUPER + 1..9` | Switch workspace |
| `SUPER + SHIFT + 1..9` | Send window to workspace |
| `SUPER + 0` | `gaming` workspace |
| `SUPER + S` | Scratchpad |
| `SUPER + F` / `SHIFT + F` | Fullscreen / maximize |
| `SUPER + SHIFT + S` | Send window to scratchpad |
| `Print` / `SHIFT + Print` / `CTRL + Print` | Screenshot region / fullscreen / window |
| `SUPER + ALT` + click-drag | Move window (mouse:272) |
| `SUPER + SHIFT + R` | Reload config |

The scripts called by these binds live in `scripts/`:

| Script | Action |
|---|---|
| `screenshot.sh` | Screenshot, saves to `~/Pictures/Screenshots`, opens Swash |
| `ocr.sh` | OCR a region → clipboard |
| `qr.sh` | Scan a QR code → clipboard |
| `lens.sh` | Google Lens (region) — **uploads to a public host, do not use it for sensitive data** |
| `focus-dir.sh` / `swap-dir.sh` | Directional focus/swap, adapting to the active layout |
| `lock-and-suspend.sh` | Lock, then suspend |

---

## Gaps, borders, blur, opacity

**`config/decorations.lua`**

```lua
gaps_in  = 5     -- spacing between windows
gaps_out = 10    -- spacing from window to screen edge
border_size = 2 -- border thickness
rounding = 12    -- corner rounding
active_opacity   = 0.9
inactive_opacity = 0.7
```

The `blur` group (`size`, `passes`) and `shadow` are in the same file. **`scale`
is not here** — that lives in `monitors.lua`.

---

## Border and window colors?

**Not in `decorations.lua`** — that file only covers gaps, rounding, opacity,
blur and shadow. Colors live in Hyprland's `col` groups. By default this skel
leaves them alone, so borders and group bars use Hyprland's own colours.

Two ways to set them:

**Follow the Noctalia theme** — Noctalia writes border colors from the active
palette into `noctalia.lua`. Both steps, and the order they have to happen in,
are in *Letting Noctalia drive the colors* below.

**Set them by hand** in `hyprland.lua`:

```lua
hl.config({
    general = {
        col = {
            active_border   = "rgb(a9aefe)",
            inactive_border = "rgb(070722)",
        },
    },
    group = {
        col = {
            border_active   = "rgb(a9aefe)",
            border_inactive = "rgb(070722)",
        },
    },
})
```

Use the `rgb(rrggbb)` form — that is what Noctalia's generated `noctalia.lua`
emits, so it is known to parse. Hyprland's own `0xAARRGGBB` notation is not
verified to be accepted here, because `hl` wraps the value rather than passing
it straight through.

That is the same `col` structure the Noctalia template emits. The block at the
end of `hyprland.lua` is a gradient variant (`colors = { primary, secondary }`
plus `angle`), which is the one that ships commented out.

---

## Letting Noctalia drive the colors

Out of the box Noctalia themes the app side only — GTK3, GTK4 and Qt. Hyprland's
own borders and your terminal keep their built-in colors. That is deliberate: a
border theme you did not ask for is harder to debug than a missing one, and the
two halves rarely settle on the same palette.

Both are opt-in.

**Easiest — use the Noctalia GUI.** Settings → `theme templates` → tick under
`Built-in templates`, then run `noctalia msg templates-apply` (or restart
Noctalia) to push it out. No file editing.

**Or edit `config.toml`** — `~/.config/noctalia/config.toml`, one line:

```toml
[theme.templates]
builtin_ids = [ "gtk3", "gtk4", "qt" ]
```

### Terminal color only

Add `"ghostty"` and change nothing else. Noctalia regenerates
`~/.config/ghostty/themes/noctalia` and restores the `theme = noctalia` line in
your `ghostty/config` by itself. Take effect with `CTRL + SHIFT + ,`
(Ghostty's default `reload_config`).

```toml
builtin_ids = [ "gtk3", "gtk4", "qt", "ghostty" ]
```

### Borders as well

Add `"hyprland"` **and** uncomment the block at the end of `hyprland.lua`. Those
are two different files and neither half works alone:

```toml
builtin_ids = [ "gtk3", "gtk4", "hyprland", "qt" ]
```

```lua
local noct = require("noctalia")
noct.apply_theme()
hl.config({
    general = {
        col = {
            active_border = {
                colors = { noct.colors.primary, noct.colors.secondary },
                angle = 45,
            },
        },
    },
})
```

Order matters:

1. Edit `config.toml` first.
2. **Wait a few seconds.** Noctalia watches that file and rewrites
   `~/.config/hypr/noctalia.lua` on its own — no restart needed.
3. Only then uncomment the block and run `hyprctl reload`.

Skipping the wait is the one way to break this: `require("noctalia")` with the
file missing is a hard config error. `hyprctl configerrors` is where you see it.
If you already hit that, comment the block back out and reload — nothing else
is damaged.

### Turning it back off

Reverse both edits. Noctalia deletes `noctalia.lua` and
`~/.config/ghostty/themes/noctalia` by itself once their ids leave `builtin_ids`,
so there is nothing left to clean up by hand. Re-comment the block in
`hyprland.lua` as well, or the require fails on the next reload.

```bash
grep builtin_ids ~/.config/noctalia/config.toml
ls ~/.config/hypr/noctalia.lua        # exists only while "hyprland" is enabled
ls ~/.config/ghostty/themes/noctalia  # exists only while "ghostty" is enabled
```

---

## Layouts and animations

- **Layout** — `config/misc.lua`. The active layout is forced to `scrolling` on
  the last line. The `dwindle` / `master` / `swapsplit` values can be changed
  there.
- **Layout presets** — `layouts/fair.lua` (automatic grid), `layouts/deck.lua`.
  Both are `dofile`d by `hyprland.lua`, so they work with no extra setup. The
  `SUPER + ALT + W` shortcut only opens the layout *picker*, and that needs the
  optional `mindset/hypr-layouts` plugin described below — without the plugin
  the shortcut does nothing and fails silently.
- **Animations** — `config/animations.lua`. Switch preset by changing one line:

  ```lua
  ANIMATION_PRESET = "bounce"   -- air, minimal, rage, smooth, ...
  ```

  The full list is in `config/animations_presets/`.

---

## Optional plugins (Noctalia)

Some settings are easier through a Noctalia plugin than by editing Lua. All of
them are optional — nothing here is required for a working session.

Source: <https://github.com/tofan79/mindset-noctalia-plugins>

These two are the recommended ones: between them they cover the fiddly parts of
this config — switching tiling layouts (including your own `deck.lua` and
`fair.lua`) and picking an animation preset — without hand-editing Lua.

| Plugin | API | What it does |
|---|---|---|
| `mindset/hypr-layouts` | 13 | Per-workspace tiling layout picker. Auto-detects your custom `layouts/*.lua` on top of the built-ins |
| `mindset/hypr-animations` | 13 | Animation preset picker, reads `config/animations_presets/` |

### Install

```bash
# register the source (Noctalia fetches it at startup)
noctalia msg plugins source add mindset git https://github.com/tofan79/mindset-noctalia-plugins.git

# enable what you want
noctalia msg plugins enable mindset/hypr-layouts
noctalia msg plugins enable mindset/hypr-animations
```

Verify with `noctalia msg plugins list | grep mindset`.

Edits inside a `.luau` plugin file hot-reload. Changes to the manifest
(`plugin.toml`) need a full config reload.

`hypr-layouts` is already wired to `SUPER + ALT + W` in `config/binds.lua` via
`mindset/hypr-layouts:panel`, so enabling that plugin is all it takes for that
shortcut to work. `hypr-animations` has no bind — open it from the Noctalia
plugin list.

### GloView (Mission Control-style overview)

GloView is **not installed by default** and its binds are commented out. The
package is in the `rakuos` repository, so it needs no extra repository:

```bash
rum install gloview-git
```

The plugin lands at `/usr/lib64/gloview.so`.


Then uncomment exactly two things:

1. **`hyprland.lua`** — the plugin load line:

   ```lua
   hl.plugin.load("/usr/lib64/gloview.so")
   ```

2. **`config/binds.lua`** — the whole guarded block at the end of the file:

   ```lua
   if hl.plugin.gloview then
       hl.bind(M .. " + SHIFT + O", hl.plugin.gloview.toggle, { description = "Toggle overview" })
       hl.bind(M .. " + ALT + O", hl.plugin.gloview.desktop, { description = "Desktop mode" })
       hl.bind(M .. " + CTRL + O", hl.plugin.gloview.allworkspaces, { description = "All workspaces" })
   end
   ```

Those two files are the only places that reference it. Uncommenting one without
the other leaves a half-enabled state: loading the plugin with no binds does
nothing visible, and registering binds for a plugin that never loads fails.

The `if hl.plugin.gloview then` guard means the binds stay harmless if you later
remove the package — they simply register nothing.

---

## Everything else (if you need it)

| File | Contents |
|---|---|
| `config/inputs.lua` | Mouse sensitivity, touchscreen gestures, natural scroll |
| `config/windowrules.lua` | Per-app rules: float, size, opacity, auto-routing to the `gaming` workspace |
| `config/autostart.lua` | Runs on Hyprland start (`noctalia`, clipboard history) |
| `config/lid.lua` | Close the laptop lid → lock and suspend |
| `config/environment.lua` | NVIDIA environment variables (commented out). If you use UWSM, edit `~/.config/uwsm/env` instead |
| `noctalia.lua` | **Not shipped, and not generated by default.** Generated only while `"hyprland"` is in `builtin_ids`, and only useful once the block at the end of `hyprland.lua` is uncommented too. Do both or neither: the require fails without the file, the file does nothing without the require. Full steps in *Letting Noctalia drive the colors*. |
| `themes/noctalia` (under `~/.config/ghostty/`) | Same deal for the terminal: appears while `"ghostty"` is in `builtin_ids`, and the `theme = noctalia` line it pairs with is restored automatically. |

---

## When something breaks

Check first:

```bash
hyprctl configerrors
```

Then reload. `SUPER + SHIFT + R` is safe to use even in a broken state.

**These files are not in git**, so `git checkout` will not bring them back. If
the config is badly broken:

```bash
# look at what a pristine copy contains, without touching your own files
ls -la /etc/skel/.config/hypr/

# restore one file by hand
cp /etc/skel/.config/hypr/config/binds.lua ~/.config/hypr/config/binds.lua
```

`/etc/skel` is the unmodified template, so it is the only reliable copy.
