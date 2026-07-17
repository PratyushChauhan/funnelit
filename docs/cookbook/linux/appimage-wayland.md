# Fix AppImage on Wayland (EGL_BAD_PARAMETER)

If the Linux AppImage exits immediately with:

```text
Could not create default EGL display: EGL_BAD_PARAMETER. Aborting...
```

you are hitting a known Tauri AppImage + Wayland mismatch: the bundle can load an older `libwayland-client` that breaks WebKit EGL init.

## Quick workaround

```bash
LD_PRELOAD=/usr/lib/libwayland-client.so ./sumeru_*.AppImage
```

On Debian/Ubuntu multiarch layouts, try:

```bash
LD_PRELOAD=/usr/lib/x86_64-linux-gnu/libwayland-client.so.0 ./sumeru_*.AppImage
```

Or force X11/XWayland:

```bash
GDK_BACKEND=x11 ./sumeru_*.AppImage
```

## Alternatives

- Use the `.deb` / `.rpm` / portable `x64.app.tar.gz` from the same release
- Prefer a Sumeru build that re-execs with the host Wayland client on AppImage + Wayland (included in current `dev`)

## Hyprland + fractional scaling

AppImages still run under X11/XWayland. For crisp UI at 1.5× on Hyprland, use the wrapper in **AppImage on Hyprland** (Linux cookbook): `force_zero_scaling`, `GDK_SCALE=2`, `GDK_DPI_SCALE=0.75`, plus the WebKit/Wayland env vars above.
