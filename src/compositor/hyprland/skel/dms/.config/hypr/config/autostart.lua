-- Auto-start config
-- if you dont use UWSM add your auto start programs here, otherwise use XDG autostart https://wiki.archlinux.org/title/XDG_Autostart

hl.on("hyprland.start", function ()
    -- Noctalia is not started from here any more.
    --
    -- DMS goes through systemctl rather than a bare `exec dms`, because
    -- dms.service declares BusName=org.freedesktop.Notifications with Type=dbus
    -- and Restart=on-failure. Started outside systemd it would own no bus name
    -- and nothing would restart it. `systemctl start` is idempotent, so this
    -- stays correct after the unit is enabled, and nothing starts twice.
    --
    -- dcal first: the calendar widget connects to it on startup.
    hl.exec_cmd("systemctl --user start dcal.service")
    hl.exec_cmd("systemctl --user start dms.service")

    hl.exec_cmd("xhost +SI:localuser:root")

    -- Clipboard history
    hl.exec_cmd("wl-paste --watch cliphist store")
end)
