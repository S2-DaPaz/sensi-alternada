#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod fire_button;
mod foreground;
mod hook;
mod scaling;
mod shared;
mod theme;

fn main() -> eframe::Result<()> {
    hook::spawn();
    foreground::spawn_watcher();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([340.0, 424.0])
            .with_resizable(false)
            .with_title("Sensibilidade alternada"),
        ..Default::default()
    };

    eframe::run_native(
        "Sensibilidade alternada",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
