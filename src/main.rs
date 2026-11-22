#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use e621_cruiser::app::E621CruiserApp;
use e621_cruiser::settings::Settings;

fn load_app_icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("../assets/e621-cruiser.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    Some(egui::IconData {
        rgba: img.as_raw().to_vec(),
        width: img.width(),
        height: img.height(),
    })
}

fn main() -> eframe::Result<()> {
    for arg in std::env::args().skip(1) {
        if arg == "--version" || arg == "-V" {
            println!("E621 Cruiser {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
    }

    let settings = Settings::load();
    let width = settings.window_width.unwrap_or(1400.0).max(800.0);
    let height = settings.window_height.unwrap_or(900.0).max(600.0);

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("E621 Cruiser")
        .with_inner_size([width, height])
        .with_min_inner_size([800.0, 560.0])
        .with_app_id("e621-cruiser");

    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "E621 Cruiser",
        options,
        Box::new(|cc| Ok(Box::new(E621CruiserApp::new(cc)))),
    )
}
