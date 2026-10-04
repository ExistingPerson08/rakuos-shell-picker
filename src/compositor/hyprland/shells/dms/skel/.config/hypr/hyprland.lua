-- RakuOS Hyprland Configuration

require("config.animations")
require("config.autostart")
require("config.decorations")
require("config.variables")
require("config.environment")
require("config.inputs")

-- GloView is opt-in: it is not installed by default and its binds are commented
-- out in config/binds.lua. Install it with "rum install gloview-git" (the
-- package is in the rakuos repository, no extra repo needed) which lands the
-- plugin at /usr/lib64/gloview.so, then uncomment BOTH this line and the
-- matching block in config/binds.lua. binds.lua only registers the shortcuts
-- when hl.plugin.gloview exists, so loading the plugin without the binds (or the
-- reverse) is a half-enabled state.
-- hl.plugin.load("/usr/lib64/gloview.so")

require("config.binds")
require("config.misc")
require("config.monitors")
require("config.windowrules")
require("config.workspaces")
require("config.lid")
dofile(os.getenv("HOME") .. "/.config/hypr/layouts/fair.lua")
dofile(os.getenv("HOME") .. "/.config/hypr/layouts/deck.lua")
