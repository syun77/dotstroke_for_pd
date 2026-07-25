use super::*;

impl DotStrokeApp {
    pub(super) fn save_history(&mut self) {
        self.history.save(&self.doc);
    }

    pub(super) fn document_is_dirty(&self) -> bool {
        serde_json::to_string(&self.doc).ok() != serde_json::to_string(&self.saved_document).ok()
    }

    pub(super) fn undo_document(&mut self) {
        if let Some(previous) = self.history.undo(&self.doc) {
            self.doc = previous;
            self.pending.clear();
            self.selected = None;
            self.selected_point = None;
            self.current_layer = self
                .current_layer
                .min(self.doc.layers.len().saturating_sub(1));
            self.status = "Undo".into();
        }
    }

    pub(super) fn redo_document(&mut self) {
        if let Some(next) = self.history.redo(&self.doc) {
            self.doc = next;
            self.pending.clear();
            self.selected = None;
            self.selected_point = None;
            self.current_layer = self
                .current_layer
                .min(self.doc.layers.len().saturating_sub(1));
            self.status = "Redo".into();
        }
    }

    pub(super) fn begin_new_document(&mut self) {
        self.new_width = self.doc.target.width.to_string();
        self.new_height = self.doc.target.height.to_string();
        self.new_dialog = true;
    }

    pub(super) fn begin_change_resolution(&mut self) {
        self.resolution_width = self.doc.target.width.clamp(8, 400);
        self.resolution_height = self.doc.target.height.clamp(8, 400);
        self.resolution_dialog = true;
    }

    pub(super) fn load_json_document(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .pick_file()
        {
            self.load_json_document_from_path(&path);
        }
    }

    pub(super) fn load_json_document_from_path(&mut self, path: &Path) {
        match io::load_document(path) {
            Ok(doc) => {
                self.save_history();
                self.saved_document = doc.clone();
                self.doc = doc;
                let path = path.to_path_buf();
                self.current_file = Some(path.clone());
                self.recent_files.retain(|recent| recent != &path);
                self.recent_files.insert(0, path.clone());
                io::save_recent_files(&self.recent_files);
                self.pending.clear();
                self.selected = None;
                self.selected_point = None;
                self.status = format!("Loaded {}", path.display());
            }
            Err(error) => self.status = format!("Failed to load JSON: {error}"),
        }
    }

    pub(super) fn restore_last_document(&mut self) {
        self.recent_files = io::load_recent_files();
        if let Some(path) = self.recent_files.first().cloned() {
            self.load_json_document_from_path(&path);
        }
    }

    pub(super) fn clear_recent_files(&mut self) {
        self.recent_files.clear();
        io::save_recent_files(&self.recent_files);
    }

    pub(super) fn save_json_document(&mut self) {
        let path = self.current_file.clone().or_else(|| {
            rfd::FileDialog::new()
                .set_file_name("document.json")
                .save_file()
        });
        if let Some(path) = path {
            match io::save_document(&path, &self.doc) {
                Ok(()) => {
                    self.saved_document = self.doc.clone();
                    self.current_file = Some(path.clone());
                    self.recent_files.retain(|recent| recent != &path);
                    self.recent_files.insert(0, path.clone());
                    io::save_recent_files(&self.recent_files);
                    self.status = format!("Saved {}", path.display());
                }
                Err(_) => self.status = "Failed to save JSON".into(),
            }
        }
    }

    pub(super) fn save_json_document_as(&mut self) {
        let default_name = self
            .current_file
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "document.json".into());
        let path = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .set_file_name(default_name)
            .save_file();
        if let Some(path) = path {
            let path = path.with_extension("json");
            match io::save_document(&path, &self.doc) {
                Ok(()) => {
                    self.saved_document = self.doc.clone();
                    self.current_file = Some(path.clone());
                    self.recent_files.retain(|recent| recent != &path);
                    self.recent_files.insert(0, path.clone());
                    io::save_recent_files(&self.recent_files);
                    self.status = format!("Saved as {}", path.display());
                }
                Err(_) => self.status = "Failed to save JSON".into(),
            }
        }
    }

    pub(super) fn export_png(&mut self) {
        let default_name = self
            .current_file
            .as_ref()
            .and_then(|path| path.file_stem())
            .map(|stem| format!("{}.png", stem.to_string_lossy()))
            .unwrap_or_else(|| "document.png".into());
        let path = rfd::FileDialog::new()
            .add_filter("PNG", &["png"])
            .set_file_name(default_name)
            .save_file();
        if let Some(path) = path {
            let path = path.with_extension("png");
            let width = self.doc.target.width.max(1) as usize;
            let height = self.doc.target.height.max(1) as usize;
            let pixels = self.pixel_preview_with_background(Color32::TRANSPARENT, true);
            match io::save_png(&path, width as u32, height as u32, &pixels) {
                Ok(()) => self.status = format!("Exported PNG {}", path.display()),
                Err(_) => self.status = "Failed to export PNG".into(),
            }
        }
    }
}
