use super::state::ReferenceImage;
use super::*;

impl DotStrokeApp {
    pub(super) fn add_reference_image(&mut self, ctx: &egui::Context, name: String, bytes: &[u8]) {
        let Ok(decoded) = image::load_from_memory(bytes) else {
            self.status = "Unsupported reference image".into();
            return;
        };
        let rgba = decoded.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let texture = ctx.load_texture(
            format!("reference-{}-{}", self.reference_images.len(), name),
            egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()),
            egui::TextureOptions::NEAREST,
        );
        self.reference_images.push(ReferenceImage {
            name,
            texture,
            size,
        });
        self.reference_selected = self.reference_images.len() - 1;
        self.reference_last_size = None;
        self.reference_window = true;
        self.status = "Reference image loaded".into();
    }

    pub(super) fn add_reference_clipboard(&mut self, ctx: &egui::Context) {
        let Ok(mut clipboard) = arboard::Clipboard::new() else {
            self.status = "Clipboard is unavailable".into();
            return;
        };
        let Ok(image) = clipboard.get_image() else {
            self.status = "Clipboard does not contain an image".into();
            return;
        };
        let rgba = match image.bytes {
            Cow::Borrowed(bytes) => bytes.to_vec(),
            Cow::Owned(bytes) => bytes,
        };
        let size = [image.width, image.height];
        let texture = ctx.load_texture(
            format!("reference-clipboard-{}", self.reference_images.len()),
            egui::ColorImage::from_rgba_unmultiplied(size, &rgba),
            egui::TextureOptions::NEAREST,
        );
        self.reference_images.push(ReferenceImage {
            name: "Clipboard image".into(),
            texture,
            size,
        });
        self.reference_selected = self.reference_images.len() - 1;
        self.reference_last_size = None;
        self.reference_window = true;
        self.status = "Clipboard image loaded".into();
    }

    pub(super) fn fit_reference_image(&mut self) {
        let Some(image) = self.reference_images.get(self.reference_selected) else {
            return;
        };
        let image_size = Vec2::new(image.size[0] as f32, image.size[1] as f32);
        self.reference_zoom = (self.reference_viewport.x / image_size.x)
            .min(self.reference_viewport.y / image_size.y)
            .clamp(0.05, 32.0);
        self.reference_pan = (self.reference_viewport - image_size * self.reference_zoom) / 2.0;
        self.reference_last_size = Some(image.size);
    }

    pub(super) fn draw_reference_preview(&mut self, ui: &mut egui::Ui) {
        let dropped_files: Vec<(String, Option<Vec<u8>>)> = ui.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| {
                    let name = file
                        .path
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "Dropped image".into());
                    (name, file.bytes.as_ref().map(|bytes| bytes.to_vec()))
                })
                .collect()
        });
        for (name, bytes) in dropped_files {
            if let Some(bytes) = bytes {
                self.add_reference_image(ui.ctx(), name, &bytes);
            } else if let Some(path) = PathBuf::from(&name).canonicalize().ok() {
                if let Ok(bytes) = fs::read(&path) {
                    self.add_reference_image(ui.ctx(), path.display().to_string(), &bytes);
                }
            }
        }
        ui.horizontal(|ui| {
            if ui.button("Open image…").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "gif", "webp"])
                    .pick_file()
                {
                    if let Ok(bytes) = fs::read(&path) {
                        self.add_reference_image(ui.ctx(), path.display().to_string(), &bytes);
                    }
                }
            }
            if ui.button("Clipboard").clicked() {
                self.add_reference_clipboard(ui.ctx());
            }
            if !self.reference_images.is_empty() {
                egui::ComboBox::from_id_salt("reference-history")
                    .selected_text(&self.reference_images[self.reference_selected].name)
                    .show_ui(ui, |ui| {
                        for (index, image) in self.reference_images.iter().enumerate() {
                            if ui
                                .selectable_value(&mut self.reference_selected, index, &image.name)
                                .clicked()
                            {
                                self.reference_last_size = None;
                            }
                        }
                    });
                if ui.button("Fit").clicked() {
                    self.fit_reference_image();
                }
                ui.label(format!("{:.0}%", self.reference_zoom * 100.0));
            }
        });
        ui.label("Drop an image here, or drag to pan. Wheel: zoom");
        let available = ui.available_size().max(Vec2::new(240.0, 180.0));
        self.reference_viewport = available;
        let (rect, response) = ui.allocate_exact_size(available, Sense::drag());
        let painter = ui.painter_at(rect).with_clip_rect(rect);
        painter.rect_filled(rect, 0.0, Color32::from_gray(42));
        if self
            .reference_images
            .get(self.reference_selected)
            .is_some_and(|image| self.reference_last_size != Some(image.size))
        {
            if let Some(image) = self.reference_images.get(self.reference_selected) {
                let image_size = Vec2::new(image.size[0] as f32, image.size[1] as f32);
                self.reference_zoom = 1.0;
                self.reference_pan = (self.reference_viewport - image_size) / 2.0;
                self.reference_last_size = Some(image.size);
            }
        }
        if let Some(image) = self.reference_images.get(self.reference_selected) {
            let size = Vec2::new(image.size[0] as f32, image.size[1] as f32) * self.reference_zoom;
            let image_rect = Rect::from_min_size(rect.left_top() + self.reference_pan, size);
            Self::draw_transparency_checkerboard(&painter, image_rect, 16.0);
            painter.image(
                image.texture.id(),
                image_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
            if response.hovered() {
                let scroll = ui.input(|input| input.smooth_scroll_delta.y);
                if scroll.abs() > 0.0 {
                    self.reference_zoom =
                        (self.reference_zoom * (1.0 + scroll.signum() * 0.1)).clamp(0.05, 64.0);
                }
            }
            if response.dragged() {
                self.reference_pan += ui.input(|input| input.pointer.delta());
            }
        } else {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Open or drop a reference image",
                egui::TextStyle::Body.resolve(ui.style()),
                Color32::LIGHT_GRAY,
            );
        }
    }
}
