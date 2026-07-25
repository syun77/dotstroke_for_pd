use crate::model::Document;
use std::{fs, path::Path};

pub fn load_document(path: &Path) -> Result<Document, String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

pub fn save_document(path: &Path, document: &Document) -> Result<(), String> {
    let text = serde_json::to_string_pretty(document).map_err(|error| error.to_string())?;
    fs::write(path, text).map_err(|error| error.to_string())
}
