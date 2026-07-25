use crate::model::Document;
use eframe::egui::Color32;
use std::{fs, path::Path};

pub fn load_document(path: &Path) -> Result<Document, String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

pub fn save_document(path: &Path, document: &Document) -> Result<(), String> {
    let text = serde_json::to_string_pretty(document).map_err(|error| error.to_string())?;
    fs::write(path, text).map_err(|error| error.to_string())
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
