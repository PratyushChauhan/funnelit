// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
mod linux_wayland;

/// Inputs: process args. Outputs: stdio MCP mode when `mcp-stdio`, else desktop UI.
fn main() {
    if std::env::args().any(|a| a == "mcp-stdio") {
        sumeru_lib::run_mcp_stdio();
        return;
    }
    #[cfg(target_os = "linux")]
    linux_wayland::maybe_reexec_with_system_wayland();
    sumeru_lib::run()
}
