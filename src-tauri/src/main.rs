// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|a| a == "mcp-stdio") {
        funnelit_lib::run_mcp_stdio();
        return;
    }
    funnelit_lib::run()
}
