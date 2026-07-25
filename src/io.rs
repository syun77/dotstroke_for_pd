use crate::model::Document;
use eframe::egui::Color32;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

const RECENT_FILES_LIMIT: usize = 10;

pub fn load_document(path: &Path) -> Result<Document, String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

pub fn save_document(path: &Path, document: &Document) -> Result<(), String> {
    let text = serde_json::to_string_pretty(document).map_err(|error| error.to_string())?;
    fs::write(path, text).map_err(|error| error.to_string())
}

fn recent_files_path() -> Option<PathBuf> {
    let config_dir = if cfg!(target_os = "windows") {
        env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
    } else {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    }?;
    Some(config_dir.join("DotStroke").join("recent_files.json"))
}

pub fn load_recent_files() -> Vec<PathBuf> {
    let Some(path) = recent_files_path() else {
        return Vec::new();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<String>>(&text)
        .unwrap_or_default()
        .into_iter()
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .take(RECENT_FILES_LIMIT)
        .collect()
}

pub fn save_recent_files(files: &[PathBuf]) {
    let Some(path) = recent_files_path() else {
        return;
    };
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let paths: Vec<String> = files
        .iter()
        .take(RECENT_FILES_LIMIT)
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    if let Ok(text) = serde_json::to_string_pretty(&paths) {
        let _ = fs::write(path, text);
    }
}

pub fn save_png(path: &Path, width: u32, height: u32, pixels: &[Color32]) -> Result<(), String> {
    let mut rgba = Vec::with_capacity(pixels.len() * 4);
    for pixel in pixels {
        let [r, g, b, a] = pixel.to_array();
        rgba.extend_from_slice(&[r, g, b, a]);
    }
    let image = image::RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| "PNG pixel buffer size does not match image dimensions".to_string())?;
    image.save(path).map_err(|error| error.to_string())
}
