mod app;
mod config;
mod editor;
mod export;
mod io;
mod model;
mod render;
mod ui;

fn main() -> eframe::Result {
    app::run()
}
