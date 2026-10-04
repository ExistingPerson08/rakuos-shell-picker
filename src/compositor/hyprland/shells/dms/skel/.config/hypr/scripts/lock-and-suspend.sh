#!/usr/bin/env bash
# DMS keeps its lock screen inside the shell, so this only works while the shell
# is up. That is no worse than before -- `noctalia msg` needed noctalia running
# too -- and systemd's Restart=on-failure brings DMS back within about a second.
dms ipc call lock lock
sleep 1
systemctl suspend --no-ask-password
