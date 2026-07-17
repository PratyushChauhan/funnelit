# Run at startup (macOS)

1. Prefer the **installed** app from Applications (not `tauri dev`)
2. Open Sumeru → **Configure**
3. Enable **Run at system startup**
4. Log out and back in (or reboot)

Sumeru starts hidden in the menu bar / tray with `--hidden`. Open from the tray to configure.

## How it works

macOS registration uses a Launch Agent (`tauri-plugin-autostart` with LaunchAgent). Enabling the toggle from a debug binary can point at a path that disappears after rebuild — turn the toggle on again from the installed app if login launch stops working.
