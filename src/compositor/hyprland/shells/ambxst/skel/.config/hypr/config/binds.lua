-- ═══════════════════════════════════════════
-- Key Bindings — Hyprland Noctalia
-- ═══════════════════════════════════════════

local M = "SUPER"
local launchPrefix = "uwsm app -- " -- if you are not using UWSM, make this empty (e.g. "")

-- ───────────────────────────────────────────
-- Core
-- ───────────────────────────────────────────
hl.bind(M .. " + SHIFT + R", hl.dsp.exec_cmd("hyprctl reload && notify-send -u low 'Hyprland reloaded'"), { description = "Reload Hyprland config" })
hl.bind(M .. " + Q", hl.dsp.window.close(), { description = "Close active window" })

-- ───────────────────────────────────────────
-- Window Grouping
-- ───────────────────────────────────────────
hl.bind(M .. " + SHIFT + G", hl.dsp.group.toggle(), { description = "Toggle window group" })
hl.bind(M .. " + CTRL + G", hl.dsp.window.move({ out_of_group = true }), { description = "Out of group" })
hl.bind(M .. " + CTRL + Bracketleft", hl.dsp.window.move({ into_group = "l" }), { description = "Into group left" })
hl.bind(M .. " + CTRL + Bracketright", hl.dsp.window.move({ into_group = "r" }), { description = "Into group right" })
hl.bind(M .. " + ALT + Bracketright", hl.dsp.window.move({ into_group = "u" }), { description = "Into group up" })
hl.bind(M .. " + ALT + Bracketleft", hl.dsp.window.move({ into_group = "d" }), { description = "Into group down" })
hl.bind(M .. " + Tab", hl.dsp.group.next(), { description = "Group next" })
hl.bind(M .. " + SHIFT + Tab", hl.dsp.group.prev(), { description = "Group prev" })
for i = 1, 9 do
	hl.bind(M .. " + CTRL + " .. i, hl.dsp.group.active({ index = i }), { description = "Group index " .. i })
end

-- ───────────────────────────────────────────
-- Scratchpad
-- ───────────────────────────────────────────
hl.bind(M .. " + ALT + S", hl.dsp.workspace.toggle_special("magic"), { description = "Toggle special workspace magic" })
hl.bind(M .. " + ALT + SHIFT + S", hl.dsp.window.move({ workspace = "special:magic" }), { description = "Send window to special workspace" })
hl.bind(M .. " + ALT + CTRL + SHIFT + S",	hl.dsp.window.move({ workspace = "previous" }), { description = "Move window out of special workspace" })

-- ───────────────────────────────────────────
-- App Launchers
-- ───────────────────────────────────────────
hl.bind(M .. " + Return", hl.dsp.exec_cmd(launchPrefix .. TERMINAL), { description = "Terminal" })
hl.bind(M .. " + E", hl.dsp.exec_cmd(launchPrefix .. FILE_MANAGER), { description = "File manager" })
hl.bind(M .. " + B", hl.dsp.exec_cmd(launchPrefix .. BROWSER), { description = "Browser (Zen)" })
hl.bind(M .. " + N", hl.dsp.exec_cmd(launchPrefix .. EDITOR), { description = "Editor" })

-- ───────────────────────────────────────────
-- Mouse Bindings
-- ───────────────────────────────────────────
hl.bind(M .. " + mouse:272", hl.dsp.window.drag(), { mouse = true, description = "Move window with mouse" })
hl.bind(M .. " + mouse:273", hl.dsp.window.resize(), { mouse = true, description = "Resize window with mouse" })
hl.bind("mouse:274", hl.dsp.window.fullscreen({ mode = "maximized", action = "toggle" }), { mouse = true, description = "Toggle Maximize (middle click)" })

-- ───────────────────────────────────────────
-- Scroll through workspaces (Super + Scroll)
-- ───────────────────────────────────────────
hl.bind(M .. " + mouse_up", hl.dsp.focus({ workspace = "e-1" }), { description = "Previous workspace (scroll)" })
hl.bind(M .. " + mouse_down", hl.dsp.focus({ workspace = "e+1" }), { description = "Next workspace (scroll)" })
hl.bind(M .. " + ALT + mouse_up", hl.dsp.window.move({ workspace = "e-1" }), { description = "Move window to previous workspace (scroll)" })
hl.bind(M .. " + ALT + mouse_down", hl.dsp.window.move({ workspace = "e+1" }), { description = "Move window to next workspace (scroll)" })
hl.bind(M .. " + CTRL + mouse_up", hl.dsp.layout("focus l"), { description = "Focus left (scrolling scroll)" })
hl.bind(M .. " + CTRL + mouse_down", hl.dsp.layout("focus r"), { description = "Focus right (scrolling scroll)" })

-- ───────────────────────────────────────────
-- Zoom Cursor (HiDPI assist)
-- ───────────────────────────────────────────
local function zoomfunction(value)
    local zoomvalue = hl.get_config("cursor:zoom_factor")
    if (zoomvalue + value) > 3.0 then
        hl.config({ cursor = { zoom_factor = 3.0 } })
    elseif (zoomvalue + value) < 1.0 then
        hl.config({ cursor = { zoom_factor = 1.0 } })
    else
        hl.config({ cursor = { zoom_factor = zoomvalue + value } })
    end
end
hl.bind(M .. " + Minus", function() zoomfunction(-0.3) end, { repeating = true, description = "Zoom out cursor" })
hl.bind(M .. " + Plus", function() zoomfunction(0.3) end, { repeating = true, description = "Zoom in cursor" })
hl.bind(M .. " + code:82", function() zoomfunction(-0.3) end, { repeating = true, description = "Zoom out cursor (keypad)" })
hl.bind(M .. " + code:86", function() zoomfunction(0.3) end, { repeating = true, description = "Zoom in cursor (keypad)" })
