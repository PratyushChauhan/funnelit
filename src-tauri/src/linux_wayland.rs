//! AppImage + Wayland startup fix for WebKit EGL_BAD_PARAMETER.

use std::{
    env,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
};

const DONE: &str = "SUMERU_WAYLAND_PRELOAD_DONE";

const CANDIDATES: &[&str] = &[
    "/usr/lib/libwayland-client.so.0",
    "/usr/lib64/libwayland-client.so.0",
    "/usr/lib/x86_64-linux-gnu/libwayland-client.so.0",
    "/usr/lib/aarch64-linux-gnu/libwayland-client.so.0",
    "/lib/x86_64-linux-gnu/libwayland-client.so.0",
    "/lib/aarch64-linux-gnu/libwayland-client.so.0",
];

/// Inputs: none.
/// Outputs: re-exec with host libwayland-client in LD_PRELOAD when needed; else unit.
///
/// Tauri AppImages built on older Ubuntu can ship a libwayland that breaks EGL
/// init on modern Wayland hosts (`EGL_BAD_PARAMETER`). Preloading the host
/// client library is the known workaround.
pub fn maybe_reexec_with_system_wayland() {
    if env::var_os(DONE).is_some() {
        return;
    }
    if env::var_os("APPIMAGE").is_none() {
        return;
    }
    if env::var_os("WAYLAND_DISPLAY").is_none() && env::var_os("WAYLAND_SOCKET").is_none() {
        return;
    }
    let preload = env::var("LD_PRELOAD").unwrap_or_default();
    if preload.split(':').any(|p| p.contains("libwayland-client")) {
        return;
    }
    let Some(lib) = find_system_wayland_client() else {
        return;
    };
    let Ok(exe) = env::current_exe() else {
        return;
    };
    let mut cmd = Command::new(&exe);
    cmd.args(env::args_os().skip(1));
    let merged = if preload.is_empty() {
        lib.display().to_string()
    } else {
        format!("{}:{preload}", lib.display())
    };
    cmd.env("LD_PRELOAD", merged);
    cmd.env(DONE, "1");
    let err = cmd.exec();
    eprintln!("sumeru: wayland preload re-exec failed: {err}");
}

/// Inputs: none. Outputs: path to host libwayland-client.so.0 when found.
fn find_system_wayland_client() -> Option<PathBuf> {
    for path in CANDIDATES {
        let p = Path::new(path);
        if p.is_file() {
            return Some(p.to_path_buf());
        }
    }
    // Last resort: ldconfig cache (Debian/Ubuntu multiarch layouts vary).
    let Ok(out) = std::process::Command::new("ldconfig").arg("-p").output() else {
        return None;
    };
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if !line.contains("libwayland-client.so.0") {
            continue;
        }
        if let Some(path) = line.split(" => ").nth(1).map(str::trim) {
            if Path::new(path).is_file() {
                return Some(PathBuf::from(path));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_are_absolute() {
        for path in CANDIDATES {
            assert!(path.starts_with('/'));
            assert!(path.contains("libwayland-client"));
        }
    }
}
