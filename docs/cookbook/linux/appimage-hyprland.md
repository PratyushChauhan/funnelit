# AppImage on Hyprland (1.5× scaling)

Tauri AppImages force `GDK_BACKEND=x11` (Wayland crashes). On Hyprland with **1.5×** monitor scaling that often looks soft unless XWayland is integer-scaled and GDK renders at 2× with DPI adjusted back to ~1.5.

## Hyprland

In `~/.config/hypr/hyprland.conf` (or a monitor config file):

```text
xwayland {
    force_zero_scaling = true
}
```

Reload Hyprland after changing it.

## Wrapper script

Save as `~/bin/sumeru` (or any path on your `PATH`), set the AppImage path, then `chmod +x`:

```bash
#!/usr/bin/env bash
# Sumeru AppImage forces GDK_BACKEND=x11 (Wayland crashes).
# On Hyprland 1.5x scaling that looks soft unless XWayland force_zero_scaling
# is on and we render at integer 2x with DPI adjusted to ~1.5.
set -euo pipefail

export LD_PRELOAD=/usr/lib/libwayland-client.so
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export GDK_SCALE=2
export GDK_DPI_SCALE=0.75

exec /path/to/sumeru.AppImage "$@"
```

Replace `/path/to/sumeru.AppImage` with your real AppImage path.

## What the env vars do

| Variable | Role |
| --- | --- |
| `LD_PRELOAD=…/libwayland-client.so` | Avoids `EGL_BAD_PARAMETER` from a mismatched bundled Wayland client |
| `WEBKIT_DISABLE_DMABUF_RENDERER=1` | More stable WebKit under XWayland |
| `WEBKIT_DISABLE_COMPOSITING_MODE=1` | Avoids compositing glitches on some GPUs |
| `GDK_SCALE=2` | Integer 2× UI scale under XWayland |
| `GDK_DPI_SCALE=0.75` | `2 × 0.75 = 1.5` effective scale to match Hyprland |

For other fractional scales, keep `GDK_SCALE` an integer and set `GDK_DPI_SCALE = desired / GDK_SCALE` (example: 1.25 → `GDK_SCALE=2`, `GDK_DPI_SCALE=0.625`).
