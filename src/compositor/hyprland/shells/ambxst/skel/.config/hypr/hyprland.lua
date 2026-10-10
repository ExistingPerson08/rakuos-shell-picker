-- RakuOS Hyprland Configuration
require("config.autostart")
require("config.monitors")
require("config.variables")
require("config.binds")
loadfile(os.getenv("HOME") .. "/.local/share/ambxst/hyprland.lua")()
