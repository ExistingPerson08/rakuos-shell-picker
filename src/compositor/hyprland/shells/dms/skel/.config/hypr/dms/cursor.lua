-- Placeholder shipped by the skel.
--
-- Unlike layout.lua, colours.lua and windowrules.lua, DMS does not rewrite this
-- one on every start -- it only writes it when the cursor setting changes. So
-- this stub is deliberate rather than temporary: an empty file is exactly what
-- this machine had before, because nothing was reading it, and pinning a cursor
-- theme here would put a value in the repository that DMS is the only thing
-- entitled to choose.
--
-- config/environment.lua requires it, so it has to exist before DMS runs: see
-- the note in layout.lua for why a failing require is not survivable. The day
-- the cursor is changed through DMS, DMS fills this in and it starts taking
-- effect on the next reload.
hl.env("HYPRCURSOR_THEME", "Bibata-Modern-Ice")
hl.env("XCURSOR_THEME", "Bibata-Modern-Ice")
hl.env("HYPRCURSOR_SIZE", "24")
hl.env("XCURSOR_SIZE", "24")
