# Run at startup (Linux)

1. Prefer an **installed / release** binary (not `tauri dev`)
2. Open Sumeru → **Configure**
3. Enable **Run at system startup**
4. Log out and back in

Sumeru starts hidden in the tray with `--hidden`.

## How it works

Autostart writes `~/.config/autostart/*.desktop` with `Exec=` pointing at the **current** binary path plus `--hidden`.

If you enabled the toggle from a debug build under `target/debug/`, re-enable it from the installed binary so the desktop entry stays valid after cleans/rebuilds.

## Check

```bash
ls ~/.config/autostart/*sumeru*
grep '^Exec=' ~/.config/autostart/*sumeru* 2>/dev/null
```
