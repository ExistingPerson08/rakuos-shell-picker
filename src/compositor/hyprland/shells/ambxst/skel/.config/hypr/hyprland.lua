-- RakuOS Hyprland Configuration
require("config.autostart")
require("config.monitors")
require("config.variables")
require("config.binds")
-- Ambxst
loadfile(os.getenv("HOME") .. "/.local/share/ambxst/hyprland.lua")()

-- OVERRIDES
-- Down here you can write or source anything that you want to override from Ambxst's settings.
