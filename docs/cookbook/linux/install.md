# Install on Linux

From a [GitHub Release](https://github.com/PratyushChauhan/sumeru/releases), pick an **amd64/x64** asset that matches your machine (`uname -m`).

| Asset | How to use |
| --- | --- |
| `*_amd64.AppImage` | `chmod +x` and run |
| `*_amd64.deb` | `sudo dpkg -i …` (Debian/Ubuntu) |
| `*.x86_64.rpm` | `sudo rpm -i …` or distro equivalent |
| `*_x64.app.tar.gz` | extract and run/`cp` to `~/.local/bin` |

Skip `aarch64` / `.dmg` assets on x86_64 Linux.

## AppImage notes

- `EGL_BAD_PARAMETER` → **Fix AppImage on Wayland**
- Soft UI on Hyprland 1.5× → **AppImage on Hyprland**
