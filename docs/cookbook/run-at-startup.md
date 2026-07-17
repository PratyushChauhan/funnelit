# Run at startup

1. Prefer an **installed / release** build (not `tauri dev`)
2. Open Sumeru → **Configure**
3. Enable **Run at system startup**
4. Log out and back in (or reboot)

Sumeru should start hidden in the tray with `--hidden`. Open from the tray to configure.

Platform details:

- **Linux** — XDG autostart desktop entry (see Linux cookbook)
- **macOS** — Launch Agent (see macOS cookbook)
