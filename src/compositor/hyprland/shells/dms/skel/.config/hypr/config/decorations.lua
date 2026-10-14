-- Look and feel configuration

hl.config({
    general = {
        gaps_in = 5,
        gaps_out = 10,
        border_size = 2,
        extend_border_grab_area = 10,
        resize_on_border = true,
        allow_tearing = true,
    },
    group = {
        groupbar = {
            font_family               = "Maple Mono NF",
            font_size                 = 10,
            gradients                 = true,
            gradient_round_only_edges = false,
            gradient_rounding         = 5,
            height                    = 18,
            indicator_height          = 0,
            gaps_in                   = 3,
            gaps_out                  = 3,
        },
    },
    decoration = {
        dim_special = 0.3,
        rounding = 12,
        active_opacity = 0.9,
        inactive_opacity = 0.7,
        fullscreen_opacity = 1.0,
        blur = {
            enabled = true,
            size = 3,
            passes = 2,
            special = true,
            new_optimizations = true,
            ignore_opacity = true,
            xray = true,
        },
        shadow = {
            enabled = true,
            range = 30,
            render_power = 3,
            color = "0x66000000",
        },
    },
})

-- DMS writes ~/.config/hypr/dms/layout.lua and colours.lua itself, so they are
-- required here rather than copied into config/: moving them would leave DMS
-- writing files nothing reads. They sit after the baseline above on purpose --
-- hl.config merges, so DMS's own gaps, border and rounding take over as soon as
-- it has run, and the values above are only what the first load after a switch
-- falls back on.
--
-- Both files exist in the skel as empty placeholders for the reason spelled out
-- in .config/hypr/dms/layout.lua: a require() that fails puts Hyprland into
-- emergency mode with no binds at all, which would also stop autostart.lua
-- from starting DMS.
require("dms.layout")
require("dms.colors")
