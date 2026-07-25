use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
mod config;
mod editor;
mod export;
mod io;
mod model;
mod render;
mod ui;

use editor::History;
use model::{Document, Style, VectorObject, DEFAULT_HEIGHT, DEFAULT_WIDTH};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

const DITHER_PATTERNS: [&str; 11] = [
    "none",
    "diagonal_line",
    "vertical_line",
    "horizontal_line",
    "screen",
    "bayer_2x2",
    "bayer_4x4",
    "bayer_8x8",
    "floyd_steinberg",
    "burkes",
    "atkinson",
];
const DITHER_ICON_DIR: &str = "assets/dither_icons";

struct ReferenceImage {
    name: String,
    texture: egui::TextureHandle,
    size: [usize; 2],
}

struct DotStrokeApp {
    doc: Document,
    tool: String,
    color: String,
    blend: String,
    width: i32,
    radius: i32,
    dither_pattern: String,
    rounding: String,
    pixel_preview: bool,
    zoom: f32,
    pan: Vec2,
    viewport_size: Vec2,
    last_fitted_target: Option<(i32, i32)>,
    pending: Vec<[f32; 2]>,
    selected: Option<(usize, usize)>,
    selected_point: Option<usize>,
    dragging_vector: Option<usize>,
    current_layer: usize,
    status: String,
    history: History,
    new_dialog: bool,
    new_width: String,
    new_height: String,
    current_file: Option<PathBuf>,
    native_menu: ui::NativeMenu,
    dither_icons: HashMap<String, egui::TextureHandle>,
    reference_window: bool,
    reference_images: Vec<ReferenceImage>,
    reference_selected: usize,
    reference_zoom: f32,
    reference_pan: Vec2,
    reference_viewport: Vec2,
    reference_last_size: Option<[usize; 2]>,
    main_was_focused: bool,
}

impl Default for DotStrokeApp {
    fn default() -> Self {
        Self {
            doc: Document::default(),
            tool: "line".into(),
            color: "black".into(),
            blend: "normal".into(),
            width: 1,
            radius: 4,
            dither_pattern: "none".into(),
            rounding: "nearest".into(),
            pixel_preview: false,
            zoom: 2.0,
            pan: Vec2::ZERO,
            viewport_size: Vec2::new(800.0, 600.0),
            last_fitted_target: None,
            pending: vec![],
            selected: None,
            selected_point: None,
            dragging_vector: None,
            current_layer: 0,
            status: "Ready".into(),
            history: History::default(),
            new_dialog: false,
            new_width: DEFAULT_WIDTH.to_string(),
            new_height: DEFAULT_HEIGHT.to_string(),
            current_file: None,
            native_menu: ui::NativeMenu::new(),
            dither_icons: HashMap::new(),
            reference_window: false,
            reference_images: Vec::new(),
            reference_selected: 0,
            reference_zoom: 1.0,
            reference_pan: Vec2::ZERO,
            reference_viewport: Vec2::new(640.0, 480.0),
            reference_last_size: None,
            main_was_focused: false,
        }
    }
}

impl DotStrokeApp {
    fn current_tool_kind(&self) -> &str {
        match self.tool.as_str() {
            "fill_rect" => "rect",
            "fill_round_rect" => "round_rect",
            "fill_circle" => "ellipse",
            "fill_polygon" => "polygon",
            _ => self.tool.as_str(),
        }
    }

    fn current_tool_fill(&self) -> bool {
        matches!(
            self.tool.as_str(),
            "fill_rect" | "fill_round_rect" | "fill_circle" | "fill_polygon"
        )
    }

    fn current_tool_label(&self) -> &str {
        match self.tool.as_str() {
            "fill_rect" => "fill rect",
            "fill_round_rect" => "fill round rect",
            "fill_circle" => "fill circle",
            "fill_polygon" => "fill polygon",
            _ => self.tool.as_str(),
        }
    }

    fn save_history(&mut self) {
        self.history.save(&self.doc);
    }

    fn undo_document(&mut self) {
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

    fn redo_document(&mut self) {
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

    fn begin_new_document(&mut self) {
        self.new_width = self.doc.target.width.to_string();
        self.new_height = self.doc.target.height.to_string();
        self.new_dialog = true;
    }

    fn load_json_document(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .pick_file()
        {
            match io::load_document(&path) {
                Ok(doc) => {
                    self.save_history();
                    self.doc = doc;
                    self.current_file = Some(path.clone());
                    self.pending.clear();
                    self.selected = None;
                    self.selected_point = None;
                    self.status = format!("Loaded {}", path.display());
                }
                Err(_) => self.status = "Failed to load JSON".into(),
            }
        }
    }

    fn save_json_document(&mut self) {
        let path = self.current_file.clone().or_else(|| {
            rfd::FileDialog::new()
                .set_file_name("document.json")
                .save_file()
        });
        if let Some(path) = path {
            match io::save_document(&path, &self.doc) {
                Ok(()) => {
                    self.current_file = Some(path.clone());
                    self.status = format!("Saved {}", path.display());
                }
                Err(_) => self.status = "Failed to save JSON".into(),
            }
        }
    }

    fn export_png(&mut self) {
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

    fn load_dither_icons(&mut self, ctx: &egui::Context) {
        let icon_dirs = [
            PathBuf::from(DITHER_ICON_DIR),
            Path::new(env!("CARGO_MANIFEST_DIR")).join(DITHER_ICON_DIR),
        ];
        for pattern in DITHER_PATTERNS {
            let Some(path) = icon_dirs
                .iter()
                .map(|dir| dir.join(format!("{pattern}.png")))
                .find(|path| path.is_file())
            else {
                continue;
            };
            let Ok(bytes) = fs::read(&path) else {
                continue;
            };
            let Ok(decoded) = image::load_from_memory(&bytes) else {
                continue;
            };
            let rgba = decoded.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            let texture = ctx.load_texture(
                format!("dither-icon-{pattern}"),
                color_image,
                egui::TextureOptions::NEAREST,
            );
            self.dither_icons.insert(pattern.into(), texture);
        }
    }

    fn add_reference_image(&mut self, ctx: &egui::Context, name: String, bytes: &[u8]) {
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

    fn add_reference_clipboard(&mut self, ctx: &egui::Context) {
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

    fn fit_reference_image(&mut self) {
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

    fn draw_reference_preview(&mut self, ui: &mut egui::Ui) {
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

    fn snap(&self, p: Pos2) -> [f32; 2] {
        match self.rounding.as_str() {
            "floor" => [p.x.floor(), p.y.floor()],
            "ceil" => [p.x.ceil(), p.y.ceil()],
            _ => [p.x.round(), p.y.round()],
        }
    }

    fn screen_to_doc(&self, rect: Rect, p: Pos2) -> Pos2 {
        render::ViewTransform {
            zoom: self.zoom,
            pan: self.pan,
        }
        .screen_to_document(rect, p)
    }

    fn doc_to_screen(&self, rect: Rect, p: [f32; 2]) -> Pos2 {
        render::ViewTransform {
            zoom: self.zoom,
            pan: self.pan,
        }
        .document_to_screen(rect, p)
    }

    fn max_zoom(&self) -> f32 {
        render::ViewTransform::max_zoom(self.viewport_size)
    }

    fn fit_canvas_to_viewport(&mut self) {
        let width = self.doc.target.width.max(1) as f32;
        let height = self.doc.target.height.max(1) as f32;
        let fit_zoom = (self.viewport_size.x / width)
            .min(self.viewport_size.y / height)
            .max(0.01);
        let canvas_size = Vec2::new(width * fit_zoom, height * fit_zoom);

        self.zoom = fit_zoom;
        self.pan = (self.viewport_size - canvas_size) / 2.0;
        self.last_fitted_target = Some((self.doc.target.width, self.doc.target.height));
    }

    fn hit_test(&self, rect: Rect, pos: Pos2) -> Option<usize> {
        let p = self.screen_to_doc(rect, pos);
        let tolerance = 8.0 / self.zoom;
        self.doc
            .layers
            .get(self.current_layer)?
            .objects
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, object)| {
                if !object.visible || object.points.is_empty() {
                    return None;
                }
                let min_x = object
                    .points
                    .iter()
                    .map(|p| p[0])
                    .fold(f32::INFINITY, f32::min)
                    - tolerance;
                let max_x = object
                    .points
                    .iter()
                    .map(|p| p[0])
                    .fold(f32::NEG_INFINITY, f32::max)
                    + tolerance;
                let min_y = object
                    .points
                    .iter()
                    .map(|p| p[1])
                    .fold(f32::INFINITY, f32::min)
                    - tolerance;
                let max_y = object
                    .points
                    .iter()
                    .map(|p| p[1])
                    .fold(f32::NEG_INFINITY, f32::max)
                    + tolerance;
                (p.x >= min_x && p.x <= max_x && p.y >= min_y && p.y <= max_y).then_some(index)
            })
    }

    fn hit_test_control_point(&self, rect: Rect, pos: Pos2) -> Option<usize> {
        let (layer_index, object_index) = self.selected?;
        let object = self
            .doc
            .layers
            .get(layer_index)?
            .objects
            .get(object_index)?;
        let hit_radius = config::interaction::CONTROL_POINT_HIT_RADIUS;
        object
            .points
            .iter()
            .enumerate()
            .map(|(index, point)| {
                let screen_point = self.doc_to_screen(rect, *point);
                let distance = screen_point.distance_sq(pos);
                (index, distance)
            })
            .filter(|(_, distance)| *distance <= hit_radius * hit_radius)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index)
    }

    fn move_selected(&mut self, delta: Vec2) {
        if let Some((layer_index, object_index)) = self.selected {
            if let Some(object) = self
                .doc
                .layers
                .get_mut(layer_index)
                .and_then(|l| l.objects.get_mut(object_index))
            {
                editor::move_object(object, delta);
            }
        }
    }

    fn move_selected_point(&mut self, point_index: usize, delta: Vec2) {
        if let Some((layer_index, object_index)) = self.selected {
            if let Some(point) = self
                .doc
                .layers
                .get_mut(layer_index)
                .and_then(|layer| layer.objects.get_mut(object_index))
                .and_then(|object| object.points.get_mut(point_index))
            {
                point[0] += delta.x;
                point[1] += delta.y;
            }
        }
    }

    fn delete_selected(&mut self) {
        if let Some((layer_index, object_index)) = self.selected.take() {
            let can_delete = self
                .doc
                .layers
                .get(layer_index)
                .map_or(false, |layer| object_index < layer.objects.len());
            if can_delete {
                self.save_history();
                self.doc.layers[layer_index].objects.remove(object_index);
                self.selected_point = None;
                self.status = "Vector deleted".into();
            }
        }
    }

    fn duplicate_selected(&mut self) {
        if let Some((layer_index, object_index)) = self.selected {
            let copy = self
                .doc
                .layers
                .get(layer_index)
                .and_then(|layer| layer.objects.get(object_index))
                .cloned();
            if let Some(copy) = copy {
                self.save_history();
                let new_index = object_index + 1;
                self.doc.layers[layer_index].objects.insert(new_index, copy);
                self.selected = Some((layer_index, new_index));
                self.selected_point = None;
                self.status = "Vector duplicated".into();
            }
        }
    }

    fn reorder_vector(&mut self, layer_index: usize, from: usize, to: usize) {
        let object_count = self
            .doc
            .layers
            .get(layer_index)
            .map_or(0, |layer| layer.objects.len());
        if from == to || from >= object_count || to >= object_count {
            return;
        }

        self.save_history();
        let layer = &mut self.doc.layers[layer_index];
        let object = layer.objects.remove(from);
        layer.objects.insert(to, object);

        if let Some((selected_layer, selected_index)) = self.selected {
            if selected_layer == layer_index {
                let updated_index = if selected_index == from {
                    to
                } else if from < to && selected_index > from && selected_index <= to {
                    selected_index - 1
                } else if to < from && selected_index >= to && selected_index < from {
                    selected_index + 1
                } else {
                    selected_index
                };
                self.selected = Some((selected_layer, updated_index));
            }
        }
        self.status = "Vector order changed".into();
    }

    fn paint_vector_drag_preview(&self, ctx: &egui::Context) {
        let Some(object_index) = self.dragging_vector else {
            return;
        };
        let Some(object) = self
            .doc
            .layers
            .get(self.current_layer)
            .and_then(|layer| layer.objects.get(object_index))
        else {
            return;
        };
        let Some(pointer_pos) = ctx.input(|input| input.pointer.latest_pos()) else {
            return;
        };

        let width = (config::ui::PREVIEW_PANEL_WIDTH - config::ui::VECTOR_ROW_ACTION_WIDTH - 24.0)
            .max(180.0);
        let rect = Rect::from_min_size(
            pointer_pos + egui::vec2(12.0, 12.0),
            Vec2::new(width, config::ui::VECTOR_ROW_HEIGHT),
        );
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::new("vector_drag_preview"),
        ));
        painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(40, 40, 40, 190));
        painter.rect_stroke(
            rect,
            4.0,
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 180)),
            egui::StrokeKind::Inside,
        );
        painter.text(
            rect.left_center() + egui::vec2(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            format!("≡ {}: {}", object_index + 1, object.kind),
            egui::FontId::proportional(config::ui::FONT_SIZE_BODY),
            Color32::from_rgba_unmultiplied(255, 255, 255, 230),
        );
        ctx.request_repaint();
    }

    fn commit_pending(&mut self, closed: bool) {
        let tool_kind = self.current_tool_kind().to_string();
        let tool_fill = self.current_tool_fill();
        let required = match tool_kind.as_str() {
            "polygon" => 3,
            _ => 2,
        };
        if self.pending.len() < required {
            return;
        }
        self.save_history();
        let layer = &mut self.doc.layers[self.current_layer];
        layer.objects.push(VectorObject {
            kind: tool_kind,
            points: self.pending.drain(..).collect(),
            closed,
            style: Style {
                color: self.color.clone(),
                blend: self.blend.clone(),
                width: self.width,
                fill: tool_fill,
                radius: self.radius,
                dither_pattern: self.dither_pattern.clone(),
                ..Style::default()
            },
            visible: true,
            ..Default::default()
        });
        self.status = "Vector added".into();
    }

    fn lua_number(value: f32) -> String {
        export::lua_number(value)
    }

    fn lua_cap_style(cap: &str) -> &'static str {
        export::lua_cap_style(cap)
    }

    fn lua_style_fields(object: &VectorObject) -> Vec<String> {
        let mut fields = vec![
            format!(
                "color = {}",
                export::lua_color_with_blend(&object.style.color, &object.style.blend)
            ),
            format!("blend = \"{}\"", object.style.blend),
            format!("cap = {}", Self::lua_cap_style(&object.style.cap)),
            format!(
                "fill = {}",
                if object.style.fill { "true" } else { "false" }
            ),
            format!("radius = {}", object.style.radius.max(0)),
        ];
        if let Some(pattern) = export::lua_dither_pattern(&object.style.dither_pattern) {
            fields.push(format!("ditherPattern = {}", pattern));
        }
        fields
    }

    fn append_lua_object_with_animation_function(
        &self,
        output: &mut String,
        object: &VectorObject,
    ) {
        if !object.visible || object.points.is_empty() {
            return;
        }

        let style_fields = Self::lua_style_fields(object).join(", ");

        match object.kind.as_str() {
            "pixel" => {
                let _ = writeln!(
                    output,
                    "drawObject(\"pixel\", {{ x = {}, y = {} }}, {{ {} }}, outline_width)",
                    Self::lua_number(object.points[0][0]),
                    Self::lua_number(object.points[0][1]),
                    style_fields
                );
            }
            "rect" if object.points.len() >= 2 => {
                let x = object.points[0][0].min(object.points[1][0]);
                let y = object.points[0][1].min(object.points[1][1]);
                let width = (object.points[0][0] - object.points[1][0]).abs();
                let height = (object.points[0][1] - object.points[1][1]).abs();
                let _ = writeln!(
                    output,
                    "drawObject(\"rect\", {{ x = {}, y = {}, width = {}, height = {} }}, {{ {} }}, outline_width)",
                    Self::lua_number(x),
                    Self::lua_number(y),
                    Self::lua_number(width),
                    Self::lua_number(height),
                    style_fields
                );
            }
            "round_rect" if object.points.len() >= 2 => {
                let x = object.points[0][0].min(object.points[1][0]);
                let y = object.points[0][1].min(object.points[1][1]);
                let width = (object.points[0][0] - object.points[1][0]).abs();
                let height = (object.points[0][1] - object.points[1][1]).abs();
                let _ = writeln!(
                    output,
                    "drawObject(\"round_rect\", {{ x = {}, y = {}, width = {}, height = {} }}, {{ {} }}, outline_width)",
                    Self::lua_number(x),
                    Self::lua_number(y),
                    Self::lua_number(width),
                    Self::lua_number(height),
                    style_fields
                );
            }
            "ellipse" if object.points.len() >= 2 => {
                let x = object.points[0][0].min(object.points[1][0]);
                let y = object.points[0][1].min(object.points[1][1]);
                let width = (object.points[0][0] - object.points[1][0]).abs();
                let height = (object.points[0][1] - object.points[1][1]).abs();
                let _ = writeln!(
                    output,
                    "drawObject(\"ellipse\", {{ x = {}, y = {}, width = {}, height = {} }}, {{ {} }}, outline_width)",
                    Self::lua_number(x),
                    Self::lua_number(y),
                    Self::lua_number(width),
                    Self::lua_number(height),
                    style_fields
                );
            }
            "polygon" if object.points.len() >= 3 => {
                let points = object
                    .points
                    .iter()
                    .flat_map(|p| [Self::lua_number(p[0]), Self::lua_number(p[1])])
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(
                    output,
                    "drawObject(\"polygon\", {{ points = {{ {} }} }}, {{ {} }}, outline_width)",
                    points, style_fields
                );
            }
            "line" | "polyline" | "path" if object.points.len() >= 2 => {
                let points = object
                    .points
                    .iter()
                    .flat_map(|p| [Self::lua_number(p[0]), Self::lua_number(p[1])])
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(
                    output,
                    "drawObject(\"path\", {{ points = {{ {} }}, closed = {} }}, {{ {} }}, outline_width)",
                    points,
                    if object.closed { "true" } else { "false" },
                    style_fields
                );
            }
            _ => {}
        }

        for child in &object.children {
            self.append_lua_object_with_animation_function(output, child);
        }
    }

    fn append_lua_object_simple(
        output: &mut String,
        object: &VectorObject,
        dither_active: &mut bool,
    ) {
        if !object.visible || object.points.is_empty() {
            return;
        }

        let point =
            |p: &[f32; 2]| format!("{}, {}", Self::lua_number(p[0]), Self::lua_number(p[1]));
        let points = |points: &[[f32; 2]]| points.iter().map(point).collect::<Vec<_>>().join(", ");
        let _ = writeln!(
            output,
            "gfx.setColor({})",
            export::lua_color_with_blend(&object.style.color, &object.style.blend)
        );
        if let Some(pattern) = export::lua_dither_pattern(&object.style.dither_pattern) {
            let _ = writeln!(output, "gfx.setDitherPattern(0.5, {})", pattern);
            *dither_active = true;
        } else if *dither_active {
            let _ = writeln!(
                output,
                "gfx.setDitherPattern(0.5, gfx.image.kDitherTypeNone)"
            );
            *dither_active = false;
        }

        match object.kind.as_str() {
            "pixel" => {
                let _ = writeln!(output, "gfx.drawPixel({})", point(&object.points[0]));
            }
            "rect" if object.points.len() >= 2 => {
                let x = object.points[0][0].min(object.points[1][0]);
                let y = object.points[0][1].min(object.points[1][1]);
                let width = (object.points[0][0] - object.points[1][0]).abs();
                let height = (object.points[0][1] - object.points[1][1]).abs();
                let _ = writeln!(output, "gfx.setLineWidth({})", object.style.width.max(1));
                if object.style.fill {
                    let _ = writeln!(
                        output,
                        "gfx.fillRect({}, {}, {}, {})",
                        Self::lua_number(x),
                        Self::lua_number(y),
                        Self::lua_number(width),
                        Self::lua_number(height)
                    );
                } else {
                    let _ = writeln!(
                        output,
                        "gfx.drawRect({}, {}, {}, {})",
                        Self::lua_number(x),
                        Self::lua_number(y),
                        Self::lua_number(width),
                        Self::lua_number(height)
                    );
                }
            }
            "round_rect" if object.points.len() >= 2 => {
                let x = object.points[0][0].min(object.points[1][0]);
                let y = object.points[0][1].min(object.points[1][1]);
                let width = (object.points[0][0] - object.points[1][0]).abs();
                let height = (object.points[0][1] - object.points[1][1]).abs();
                let function = if object.style.fill {
                    "fillRoundRect"
                } else {
                    "drawRoundRect"
                };
                let _ = writeln!(output, "gfx.setLineWidth({})", object.style.width.max(1));
                let _ = writeln!(
                    output,
                    "gfx.{}({}, {}, {}, {}, {})",
                    function,
                    Self::lua_number(x),
                    Self::lua_number(y),
                    Self::lua_number(width),
                    Self::lua_number(height),
                    object.style.radius.max(0)
                );
            }
            "ellipse" if object.points.len() >= 2 => {
                let x = object.points[0][0].min(object.points[1][0]);
                let y = object.points[0][1].min(object.points[1][1]);
                let width = (object.points[0][0] - object.points[1][0]).abs();
                let height = (object.points[0][1] - object.points[1][1]).abs();
                let _ = writeln!(output, "gfx.setLineWidth({})", object.style.width.max(1));
                let function = if object.style.fill {
                    "fillEllipseInRect"
                } else {
                    "drawEllipseInRect"
                };
                let _ = writeln!(
                    output,
                    "gfx.{}({}, {}, {}, {})",
                    function,
                    Self::lua_number(x),
                    Self::lua_number(y),
                    Self::lua_number(width),
                    Self::lua_number(height)
                );
            }
            "polygon" if object.points.len() >= 3 => {
                let _ = writeln!(output, "gfx.setLineWidth({})", object.style.width.max(1));
                let _ = writeln!(
                    output,
                    "gfx.setLineCapStyle({})",
                    Self::lua_cap_style(&object.style.cap)
                );
                let args = points(&object.points);
                if object.style.fill {
                    let _ = writeln!(output, "gfx.fillPolygon({})", args);
                }
                let _ = writeln!(output, "gfx.drawPolygon({})", args);
            }
            "line" | "polyline" | "path" if object.points.len() >= 2 => {
                let _ = writeln!(output, "gfx.setLineWidth({})", object.style.width.max(1));
                let _ = writeln!(
                    output,
                    "gfx.setLineCapStyle({})",
                    Self::lua_cap_style(&object.style.cap)
                );
                for pair in object.points.windows(2) {
                    let _ = writeln!(
                        output,
                        "gfx.drawLine({}, {}, {}, {})",
                        Self::lua_number(pair[0][0]),
                        Self::lua_number(pair[0][1]),
                        Self::lua_number(pair[1][0]),
                        Self::lua_number(pair[1][1])
                    );
                }
                if object.closed && object.points.len() > 2 {
                    let first = &object.points[0];
                    let last = object.points.last().unwrap();
                    let _ = writeln!(
                        output,
                        "gfx.drawLine({}, {}, {}, {})",
                        Self::lua_number(last[0]),
                        Self::lua_number(last[1]),
                        Self::lua_number(first[0]),
                        Self::lua_number(first[1])
                    );
                }
            }
            _ => {}
        }

        for child in &object.children {
            Self::append_lua_object_simple(output, child, dither_active);
        }
    }

    fn last_visible_lua_color(object: &VectorObject) -> Option<&str> {
        if !object.visible || object.points.is_empty() {
            return None;
        }
        let mut color = object.style.color.as_str();
        for child in &object.children {
            if let Some(child_color) = Self::last_visible_lua_color(child) {
                color = child_color;
            }
        }
        Some(color)
    }

    #[allow(dead_code)]
    fn collect_animation_kinds(object: &VectorObject, kinds: &mut HashSet<String>) {
        if object.visible && !object.points.is_empty() {
            kinds.insert(object.kind.clone());
        }
        for child in &object.children {
            Self::collect_animation_kinds(child, kinds);
        }
    }

    #[allow(dead_code)]
    fn animation_primitive_lua(&self) -> String {
        let mut kinds = HashSet::new();
        for layer in &self.doc.layers {
            if layer.visible {
                for object in &layer.objects {
                    Self::collect_animation_kinds(object, &mut kinds);
                }
            }
        }

        let mut output = format!(
            "local gfx <const> = playdate.graphics\n\nlocal function drawPrimitive(kind, params, style, outline_width)\n    params = params or {{}}\n    style = style or {{}}\n    if outline_width == nil then\n        outline_width = {}\n    end\n\n    if style.blend == \"xor\" then\n        gfx.setColor(gfx.kColorXOR)\n    else\n        gfx.setColor(style.color or gfx.kColorBlack)\n    end\n    if style.ditherPattern then\n        gfx.setDitherPattern(0.5, style.ditherPattern)\n    end\n\n",
            0
        );
        let mut first_branch = true;
        let mut branch = |kind: &str| {
            let keyword = if first_branch { "if" } else { "elseif" };
            first_branch = false;
            format!("    {} kind == \"{}\" then\n", keyword, kind)
        };

        if kinds.contains("pixel") {
            output.push_str(&branch("pixel"));
            output.push_str("        gfx.drawPixel(params.x or 0, params.y or 0)\n");
        }
        if kinds.contains("rect") {
            output.push_str(&branch("rect"));
            output.push_str("        if style.fill then\n            gfx.fillRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0)\n        end\n        if outline_width > 0 then\n            gfx.setLineWidth(outline_width)\n            gfx.drawRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0)\n        end\n");
        }
        if kinds.contains("round_rect") {
            output.push_str(&branch("round_rect"));
            output.push_str("        local radius = style.radius or 0\n        if style.fill then\n            gfx.fillRoundRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0, radius)\n        end\n        if outline_width > 0 then\n            gfx.setLineWidth(outline_width)\n            gfx.drawRoundRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0, radius)\n        end\n");
        }
        if kinds.contains("ellipse") {
            output.push_str(&branch("ellipse"));
            output.push_str("        if style.fill then\n            gfx.fillEllipseInRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0)\n        end\n        if outline_width > 0 then\n            gfx.setLineWidth(outline_width)\n            gfx.drawEllipseInRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0)\n        end\n");
        }
        if kinds.contains("polygon") {
            output.push_str(&branch("polygon"));
            output.push_str("        local points = params.points or {}\n        if style.fill then\n            gfx.fillPolygon(table.unpack(points))\n        end\n        if outline_width > 0 then\n            gfx.setLineWidth(outline_width)\n            gfx.setLineCapStyle(style.cap or gfx.kLineCapStyleButt)\n            gfx.drawPolygon(table.unpack(points))\n        end\n");
        }
        if kinds.contains("path") {
            output.push_str(&branch("path"));
            output.push_str("        local points = params.points or {}\n        if outline_width > 0 then\n            gfx.setLineWidth(outline_width)\n            gfx.setLineCapStyle(style.cap or gfx.kLineCapStyleButt)\n            for i = 1, #points - 2, 2 do\n                gfx.drawLine(points[i], points[i + 1], points[i + 2], points[i + 3])\n            end\n            if params.closed and #points > 4 then\n                gfx.drawLine(points[#points - 1], points[#points], points[1], points[2])\n            end\n        end\n");
        }
        if first_branch {
            output.push_str("end\n\n");
        } else {
            output.push_str("    end\nend\n\n");
        }
        output
    }

    fn append_animation_object_inline(&self, output: &mut String, object: &VectorObject) {
        if !object.visible || object.points.is_empty() {
            return;
        }

        let style = format!("{{ {} }}", Self::lua_style_fields(object).join(", "));
        output.push_str("    do\n");
        let output_kind = match object.kind.as_str() {
            "line" | "polyline" | "path" => "path",
            kind => kind,
        };
        let _ = writeln!(output, "        local kind = \"{}\"", output_kind);

        match object.kind.as_str() {
            "pixel" => {
                let _ = writeln!(
                    output,
                    "        local params = {{ x = {}, y = {} }}",
                    Self::lua_number(object.points[0][0]),
                    Self::lua_number(object.points[0][1])
                );
            }
            "rect" | "round_rect" | "ellipse" if object.points.len() >= 2 => {
                let x = object.points[0][0].min(object.points[1][0]);
                let y = object.points[0][1].min(object.points[1][1]);
                let width = (object.points[0][0] - object.points[1][0]).abs();
                let height = (object.points[0][1] - object.points[1][1]).abs();
                let _ = writeln!(
                    output,
                    "        local params = {{ x = {}, y = {}, width = {}, height = {} }}",
                    Self::lua_number(x),
                    Self::lua_number(y),
                    Self::lua_number(width),
                    Self::lua_number(height)
                );
            }
            "polygon" | "path" | "line" | "polyline" if object.points.len() >= 2 => {
                let points = object
                    .points
                    .iter()
                    .flat_map(|p| [Self::lua_number(p[0]), Self::lua_number(p[1])])
                    .collect::<Vec<_>>()
                    .join(", ");
                let closed = if object.closed { "true" } else { "false" };
                let _ = writeln!(
                    output,
                    "        local params = {{ points = {{ {} }}, closed = {} }}",
                    points, closed
                );
            }
            _ => {
                output.push_str("    end\n");
                return;
            }
        }

        let _ = writeln!(output, "        local style = {}", style);
        output.push_str(
            "        if style.blend == \"xor\" then\n            gfx.setColor(gfx.kColorXOR)\n        else\n            gfx.setColor(style.color or gfx.kColorBlack)\n        end\n        if style.ditherPattern then\n            gfx.setDitherPattern(0.5, style.ditherPattern)\n        else\n            gfx.setDitherPattern(0.5, gfx.image.kDitherTypeNone)\n        end\n",
        );
        output.push_str(
            "        if kind == \"rect\" or kind == \"round_rect\" or kind == \"ellipse\" then\n            local shape_center_x = (params.x or 0) + (params.width or 0) / 2\n            local shape_center_y = (params.y or 0) + (params.height or 0) / 2\n            local scaled_center_x = rotation_center_x + (shape_center_x - rotation_center_x) * scale_x\n            local scaled_center_y = rotation_center_y + (shape_center_y - rotation_center_y) * scale_y\n            local transformed_center_x = rotation_center_x + (scaled_center_x - rotation_center_x) * rotation_cos - (scaled_center_y - rotation_center_y) * rotation_sin + offset_x\n            local transformed_center_y = rotation_center_y + (scaled_center_x - rotation_center_x) * rotation_sin + (scaled_center_y - rotation_center_y) * rotation_cos + offset_y\n            params.x = transformed_center_x - (params.width or 0) * math.abs(scale_x) / 2\n            params.y = transformed_center_y - (params.height or 0) * math.abs(scale_y) / 2\n            params.width = (params.width or 0) * math.abs(scale_x)\n            params.height = (params.height or 0) * math.abs(scale_y)\n        end\n",
        );

        match object.kind.as_str() {
            "pixel" => {
                output.push_str(
                    "        if kind == \"pixel\" then\n            local px = params.x or 0\n            local py = params.y or 0\n            local scaled_x = rotation_center_x + (px - rotation_center_x) * scale_x\n            local scaled_y = rotation_center_y + (py - rotation_center_y) * scale_y\n            local transformed_x = rotation_center_x + (scaled_x - rotation_center_x) * rotation_cos - (scaled_y - rotation_center_y) * rotation_sin + offset_x\n            local transformed_y = rotation_center_y + (scaled_x - rotation_center_x) * rotation_sin + (scaled_y - rotation_center_y) * rotation_cos + offset_y\n            gfx.drawPixel(transformed_x, transformed_y)\n        end\n",
                );
            }
            "rect" => {
                output.push_str(
                    "        if kind == \"rect\" then\n            if style.fill then\n                gfx.fillRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0)\n            end\n            if outline_width > 0 then\n                gfx.setLineWidth(outline_width)\n                gfx.drawRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0)\n            end\n        end\n",
                );
            }
            "round_rect" => {
                output.push_str(
                    "        if kind == \"round_rect\" then\n            local radius = style.radius or 0\n            if style.fill then\n                gfx.fillRoundRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0, radius)\n            end\n            if outline_width > 0 then\n                gfx.setLineWidth(outline_width)\n                gfx.drawRoundRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0, radius)\n            end\n        end\n",
                );
            }
            "ellipse" => {
                output.push_str(
                    "        if kind == \"ellipse\" then\n            if style.fill then\n                gfx.fillEllipseInRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0)\n            end\n            if outline_width > 0 then\n                gfx.setLineWidth(outline_width)\n                gfx.drawEllipseInRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0)\n            end\n        end\n",
                );
            }
            "polygon" => {
                output.push_str(
                    "        if kind == \"polygon\" then\n            local points = {}\n            for i = 1, #(params.points or {}), 2 do\n                local px = params.points[i]\n                local py = params.points[i + 1]\n                local scaled_x = rotation_center_x + (px - rotation_center_x) * scale_x\n                local scaled_y = rotation_center_y + (py - rotation_center_y) * scale_y\n                points[i] = rotation_center_x + (scaled_x - rotation_center_x) * rotation_cos - (scaled_y - rotation_center_y) * rotation_sin + offset_x\n                points[i + 1] = rotation_center_y + (scaled_x - rotation_center_x) * rotation_sin + (scaled_y - rotation_center_y) * rotation_cos + offset_y\n            end\n            if style.fill then\n                gfx.fillPolygon(table.unpack(points))\n            end\n            if outline_width > 0 then\n                gfx.setLineWidth(outline_width)\n                gfx.setLineCapStyle(style.cap or gfx.kLineCapStyleButt)\n                gfx.drawPolygon(table.unpack(points))\n            end\n        end\n",
                );
            }
            "path" | "line" | "polyline" => {
                output.push_str(
                    "        if kind == \"path\" then\n            local points = {}\n            for i = 1, #(params.points or {}), 2 do\n                local px = params.points[i]\n                local py = params.points[i + 1]\n                local scaled_x = rotation_center_x + (px - rotation_center_x) * scale_x\n                local scaled_y = rotation_center_y + (py - rotation_center_y) * scale_y\n                points[i] = rotation_center_x + (scaled_x - rotation_center_x) * rotation_cos - (scaled_y - rotation_center_y) * rotation_sin + offset_x\n                points[i + 1] = rotation_center_y + (scaled_x - rotation_center_x) * rotation_sin + (scaled_y - rotation_center_y) * rotation_cos + offset_y\n            end\n            if outline_width > 0 then\n                gfx.setLineWidth(outline_width)\n                gfx.setLineCapStyle(style.cap or gfx.kLineCapStyleButt)\n                for i = 1, #points - 2, 2 do\n                    gfx.drawLine(points[i], points[i + 1], points[i + 2], points[i + 3])\n                end\n                if params.closed and #points > 4 then\n                    gfx.drawLine(points[#points - 1], points[#points], points[1], points[2])\n                end\n            end\n        end\n",
                );
            }
            _ => {}
        }
        output.push_str("    end\n");

        for child in &object.children {
            self.append_animation_object_inline(output, child);
        }
    }

    fn animation_single_function_lua(&self) -> String {
        let mut output = format!(
            "local gfx <const> = playdate.graphics\n\nlocal function drawPrimitive(offset_x, offset_y, rotation, scale_x, scale_y, outline_width)\n    if offset_x == nil then\n        offset_x = 0\n    end\n    if offset_y == nil then\n        offset_y = 0\n    end\n    if rotation == nil then\n        rotation = 0\n    end\n    if scale_x == nil then\n        scale_x = 1\n    end\n    if scale_y == nil then\n        scale_y = 1\n    end\n    if outline_width == nil then\n        outline_width = {}\n    end\n    local rotation_radians = math.rad(rotation)\n    local rotation_cos = math.cos(rotation_radians)\n    local rotation_sin = math.sin(rotation_radians)\n    local rotation_center_x = {}\n    local rotation_center_y = {}\n\n",
            0,
            self.doc.target.width as f32 / 2.0,
            self.doc.target.height as f32 / 2.0
        );
        for layer in &self.doc.layers {
            if layer.visible {
                for object in &layer.objects {
                    self.append_animation_object_inline(&mut output, object);
                }
            }
        }
        let last_color = self
            .doc
            .layers
            .iter()
            .filter(|layer| layer.visible)
            .flat_map(|layer| layer.objects.iter())
            .filter_map(Self::last_visible_lua_color)
            .last();
        if last_color == Some("white") {
            output.push_str("    gfx.setColor(gfx.kColorBlack)\n");
        }
        output.push_str("end\n");
        output
    }

    fn playdate_lua(&self, animation: bool) -> String {
        if animation {
            return self.animation_single_function_lua();
        }

        let mut output = if animation {
            String::from(
                "local gfx <const> = playdate.graphics\n\nlocal function drawPrimitive(kind, params, style)\n    params = params or {}\n    style = style or {}\n\n    if style.blend == \"xor\" then\n        gfx.setColor(gfx.kColorXOR)\n    else\n        gfx.setColor(style.color or gfx.kColorBlack)\n    end\n    if style.ditherPattern then\n        gfx.setDitherPattern(0.5, style.ditherPattern)\n    end\n\n    local lineWidth = style.width or 1\n    local lineCap = style.cap or gfx.kLineCapStyleButt\n\n    if kind == \"pixel\" then\n        gfx.drawPixel(params.x or 0, params.y or 0)\n    elseif kind == \"rect\" then\n        gfx.setLineWidth(lineWidth)\n        if style.fill then\n            gfx.fillRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0)\n        else\n            gfx.drawRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0)\n        end\n    elseif kind == \"round_rect\" then\n        gfx.setLineWidth(lineWidth)\n        local radius = style.radius or 0\n        if style.fill then\n            gfx.fillRoundRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0, radius)\n        else\n            gfx.drawRoundRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0, radius)\n        end\n    elseif kind == \"ellipse\" then\n        gfx.setLineWidth(lineWidth)\n        if style.fill then\n            gfx.fillEllipseInRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0)\n        else\n            gfx.drawEllipseInRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0)\n        end\n    elseif kind == \"polygon\" then\n        local points = params.points or {}\n        gfx.setLineWidth(lineWidth)\n        gfx.setLineCapStyle(lineCap)\n        if style.fill then\n            gfx.fillPolygon(table.unpack(points))\n        end\n        gfx.drawPolygon(table.unpack(points))\n    elseif kind == \"path\" then\n        local points = params.points or {}\n        gfx.setLineWidth(lineWidth)\n        gfx.setLineCapStyle(lineCap)\n        for i = 1, #points - 2, 2 do\n            gfx.drawLine(points[i], points[i + 1], points[i + 2], points[i + 3])\n        end\n        if params.closed and #points > 4 then\n            gfx.drawLine(points[#points - 1], points[#points], points[1], points[2])\n        end\n    end\nend\n\n",
            )
        } else {
            String::from("local gfx <const> = playdate.graphics\n")
        };
        if animation {
            let zero_outline_guard = format!(
                "    if lineWidth <= 0 and kind ~= \"pixel\" then\n        if style.fill then\n            if kind == \"rect\" then\n                gfx.fillRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0)\n            elseif kind == \"round_rect\" then\n                gfx.fillRoundRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0, style.radius or 0)\n            elseif kind == \"ellipse\" then\n                gfx.fillEllipseInRect(params.x or 0, params.y or 0, params.width or 0, params.height or 0)\n            elseif kind == \"polygon\" then\n                gfx.fillPolygon(table.unpack(params.points or {{}}))\n            end\n        end\n        return\n    end\n\n"
            );
            output = output.replace(
                "    local lineWidth = style.width or 1\n",
                &format!(
                    "    local lineWidth = style.width or 1\n{}",
                    zero_outline_guard
                ),
            );
        }
        let mut dither_active = false;
        for layer in &self.doc.layers {
            if layer.visible {
                for object in &layer.objects {
                    if animation {
                        self.append_lua_object_with_animation_function(&mut output, object);
                    } else {
                        Self::append_lua_object_simple(&mut output, object, &mut dither_active);
                    }
                }
            }
        }
        if !animation {
            let last_color = self
                .doc
                .layers
                .iter()
                .filter(|layer| layer.visible)
                .flat_map(|layer| layer.objects.iter())
                .filter_map(Self::last_visible_lua_color)
                .last();
            if last_color == Some("white") {
                let _ = writeln!(output, "gfx.setColor(gfx.kColorBlack)");
            }
        }
        output
    }

    fn draw_object(&self, painter: &egui::Painter, rect: Rect, object: &VectorObject) {
        self.draw_object_at(painter, rect, object, self.zoom, self.pan);
    }

    fn draw_object_at(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        object: &VectorObject,
        zoom: f32,
        pan: Vec2,
    ) {
        if !object.visible || object.points.is_empty() {
            return;
        }
        let pts: Vec<Pos2> = object
            .points
            .iter()
            .map(|p| {
                Pos2::new(
                    rect.left() + pan.x + p[0] * zoom,
                    rect.top() + pan.y + p[1] * zoom,
                )
            })
            .collect();
        let color = match object.style.color.as_str() {
            "white" => Color32::WHITE,
            "clear" => config::colors::clear_color(),
            _ => Color32::BLACK,
        };
        let stroke = Stroke::new((object.style.width.max(1) as f32) * zoom, color);
        if object.style.dither_pattern != "none" {
            self.draw_object_dithered_at(painter, rect, object, &pts, stroke, color, zoom, pan);
            return;
        }
        match object.kind.as_str() {
            "pixel" => {
                painter.circle_filled(
                    pts[0],
                    (object.style.width.max(1) as f32 * zoom).max(1.0),
                    color,
                );
            }
            "rect" if pts.len() >= 2 => {
                let r = Rect::from_two_pos(pts[0], pts[1]);
                if object.style.fill {
                    painter.rect_filled(r, 0.0, color);
                } else {
                    painter.rect_stroke(r, 0.0, stroke, egui::StrokeKind::Middle);
                }
            }
            "round_rect" if pts.len() >= 2 => {
                let r = Rect::from_two_pos(pts[0], pts[1]);
                let radius = object.style.radius.max(0) as f32 * zoom;
                if object.style.fill {
                    painter.rect_filled(r, radius, color);
                } else {
                    painter.rect_stroke(r, radius, stroke, egui::StrokeKind::Middle);
                }
            }
            "ellipse" if pts.len() >= 2 => {
                let r = Rect::from_two_pos(pts[0], pts[1]);
                if object.style.fill {
                    painter.add(egui::Shape::ellipse_filled(
                        r.center(),
                        Vec2::new(r.width() / 2.0, r.height() / 2.0),
                        color,
                    ));
                }
                painter.add(egui::Shape::ellipse_stroke(
                    r.center(),
                    Vec2::new(r.width() / 2.0, r.height() / 2.0),
                    stroke,
                ));
            }
            "polygon" if pts.len() >= 3 => {
                if object.style.fill {
                    painter.add(egui::Shape::convex_polygon(pts.clone(), color, stroke));
                } else {
                    painter.add(egui::Shape::closed_line(pts.clone(), stroke));
                }
            }
            _ => {
                for pair in pts.windows(2) {
                    painter.line_segment([pair[0], pair[1]], stroke);
                }
                if object.closed && pts.len() > 2 {
                    painter.line_segment([*pts.last().unwrap(), pts[0]], stroke);
                }
            }
        }
    }

    fn point_in_polygon_pos2(sample: Pos2, points: &[Pos2]) -> bool {
        if points.len() < 3 {
            return false;
        }
        let mut inside = false;
        let mut previous = *points.last().unwrap();
        for &current in points {
            let intersects = ((current.y > sample.y) != (previous.y > sample.y))
                && (sample.x
                    < (previous.x - current.x) * (sample.y - current.y)
                        / ((previous.y - current.y).abs().max(f32::EPSILON))
                        + current.x);
            if intersects {
                inside = !inside;
            }
            previous = current;
        }
        inside
    }

    fn vector_dither_allows_sample(
        &self,
        pattern: &str,
        sample: Pos2,
        rect: Rect,
        zoom: f32,
        pan: Vec2,
    ) -> bool {
        let cell = zoom.max(1.0);
        let gx = ((sample.x - rect.left() - pan.x) / cell).floor();
        let gy = ((sample.y - rect.top() - pan.y) / cell).floor();
        if gx < 0.0 || gy < 0.0 {
            return false;
        }
        Self::dither_allows_pixel(pattern, gx as usize, gy as usize)
    }

    fn draw_object_dithered_at(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        object: &VectorObject,
        pts: &[Pos2],
        stroke: Stroke,
        color: Color32,
        zoom: f32,
        pan: Vec2,
    ) {
        if pts.is_empty() {
            return;
        }
        let min_x = pts
            .iter()
            .map(|point| point.x)
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(rect.left().floor()) as i32;
        let max_x = pts
            .iter()
            .map(|point| point.x)
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil()
            .min(rect.right().ceil()) as i32;
        let min_y = pts
            .iter()
            .map(|point| point.y)
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(rect.top().floor()) as i32;
        let max_y = pts
            .iter()
            .map(|point| point.y)
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil()
            .min(rect.bottom().ceil()) as i32;
        if min_x > max_x || min_y > max_y {
            return;
        }

        // Adding one painter shape per pixel is very expensive in egui. Build a
        // single mesh instead; the rasterization work stays the same, but the
        // number of shapes submitted to the painter drops from O(pixels) to 1.
        let mut mesh = egui::Mesh::default();
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let sample = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
                let stroke_radius = stroke.width / 2.0;
                let drawn = match object.kind.as_str() {
                    "pixel" => sample.distance(pts[0]) <= stroke.width.max(1.0),
                    "rect" if pts.len() >= 2 => {
                        let left = pts[0].x.min(pts[1].x);
                        let right = pts[0].x.max(pts[1].x);
                        let top = pts[0].y.min(pts[1].y);
                        let bottom = pts[0].y.max(pts[1].y);
                        let inside = sample.x >= left
                            && sample.x <= right
                            && sample.y >= top
                            && sample.y <= bottom;
                        let edge_distance = (sample.x - left)
                            .min(right - sample.x)
                            .min((sample.y - top).min(bottom - sample.y));
                        inside && (object.style.fill || edge_distance <= stroke_radius)
                    }
                    "round_rect" if pts.len() >= 2 => {
                        let left = pts[0].x.min(pts[1].x);
                        let right = pts[0].x.max(pts[1].x);
                        let top = pts[0].y.min(pts[1].y);
                        let bottom = pts[0].y.max(pts[1].y);
                        let half_w = ((right - left) / 2.0).max(0.0);
                        let half_h = ((bottom - top) / 2.0).max(0.0);
                        let radius =
                            (object.style.radius.max(0) as f32 * zoom).min(half_w.min(half_h));
                        let center_x = (left + right) / 2.0;
                        let center_y = (top + bottom) / 2.0;
                        let qx = (sample.x - center_x).abs() - (half_w - radius);
                        let qy = (sample.y - center_y).abs() - (half_h - radius);
                        let outside = qx.max(0.0).hypot(qy.max(0.0));
                        let inside = qx.max(qy).min(0.0);
                        let signed_distance = outside + inside - radius;
                        if object.style.fill {
                            signed_distance <= 0.0
                        } else {
                            signed_distance.abs() <= stroke_radius
                        }
                    }
                    "ellipse" if pts.len() >= 2 => {
                        let left = pts[0].x.min(pts[1].x);
                        let right = pts[0].x.max(pts[1].x);
                        let top = pts[0].y.min(pts[1].y);
                        let bottom = pts[0].y.max(pts[1].y);
                        let rx = (right - left) / 2.0;
                        let ry = (bottom - top) / 2.0;
                        if rx <= 0.0 || ry <= 0.0 {
                            false
                        } else {
                            let dx = (sample.x - (left + right) / 2.0) / rx;
                            let dy = (sample.y - (top + bottom) / 2.0) / ry;
                            let value = dx * dx + dy * dy;
                            (object.style.fill && value <= 1.0)
                                || (!object.style.fill
                                    && (value - 1.0).abs() * rx.min(ry) <= stroke_radius)
                        }
                    }
                    "polygon" if pts.len() >= 3 => {
                        let filled = object.style.fill && Self::point_in_polygon_pos2(sample, pts);
                        let edge = pts
                            .iter()
                            .zip(pts.iter().cycle().skip(1))
                            .take(pts.len())
                            .any(|(start, end)| {
                                editor::distance_to_segment(sample, *start, *end) <= stroke_radius
                            });
                        filled || edge
                    }
                    _ if pts.len() >= 2 => {
                        pts.windows(2).any(|pair| {
                            editor::distance_to_segment(sample, pair[0], pair[1]) <= stroke_radius
                        }) || (object.closed
                            && editor::distance_to_segment(sample, *pts.last().unwrap(), pts[0])
                                <= stroke_radius)
                    }
                    _ => false,
                };
                if drawn
                    && self.vector_dither_allows_sample(
                        &object.style.dither_pattern,
                        sample,
                        rect,
                        zoom,
                        pan,
                    )
                {
                    let pixel =
                        Rect::from_min_size(Pos2::new(x as f32, y as f32), Vec2::splat(1.0));
                    mesh.add_colored_rect(pixel, color);
                }
            }
        }
        if !mesh.indices.is_empty() {
            painter.add(egui::Shape::mesh(mesh));
        }
    }

    fn draw_tool_preview(&self, painter: &egui::Painter, rect: Rect, cursor: Pos2) {
        let tool_kind = self.current_tool_kind();
        let fill_shape = self.current_tool_fill();
        if tool_kind == "select" {
            return;
        }

        let cursor = self.snap(self.screen_to_doc(rect, cursor));
        let cursor = self.doc_to_screen(rect, cursor);
        let preview_color = config::colors::preview_stroke();
        let preview_fill = config::colors::preview_fill();
        let preview_stroke = Stroke::new(
            (self.width.max(1) as f32 * self.zoom).max(1.0),
            preview_color,
        );

        match tool_kind {
            "pixel" => {
                painter.circle_filled(
                    cursor,
                    (self.width.max(1) as f32 * self.zoom).max(1.0),
                    preview_fill,
                );
                painter.circle_stroke(cursor, preview_stroke.width, preview_stroke);
            }
            "line" | "polyline" | "path" => {
                if let Some(point) = self.pending.last() {
                    painter
                        .line_segment([self.doc_to_screen(rect, *point), cursor], preview_stroke);
                } else {
                    painter.circle_stroke(cursor, 6.0, preview_stroke);
                }
            }
            "polygon" | "fill_polygon" => {
                if let Some(point) = self.pending.last() {
                    painter
                        .line_segment([self.doc_to_screen(rect, *point), cursor], preview_stroke);
                    if fill_shape && self.pending.len() >= 2 {
                        let mut polygon_points = self
                            .pending
                            .iter()
                            .map(|point| self.doc_to_screen(rect, *point))
                            .collect::<Vec<_>>();
                        polygon_points.push(cursor);
                        painter.add(egui::Shape::convex_polygon(
                            polygon_points,
                            preview_fill,
                            Stroke::NONE,
                        ));
                    }
                    if self.pending.len() >= 2 {
                        painter.line_segment(
                            [cursor, self.doc_to_screen(rect, self.pending[0])],
                            preview_stroke,
                        );
                    }
                } else {
                    painter.circle_stroke(cursor, 6.0, preview_stroke);
                }
            }
            "rect" | "round_rect" | "ellipse" => {
                if let Some(point) = self.pending.first() {
                    let start = self.doc_to_screen(rect, *point);
                    let bounds = Rect::from_two_pos(start, cursor);
                    if matches!(tool_kind, "rect" | "round_rect") && fill_shape {
                        let radius = if tool_kind == "round_rect" {
                            self.radius.max(0) as f32 * self.zoom
                        } else {
                            0.0
                        };
                        painter.rect_filled(bounds, radius, preview_fill);
                    }
                    if tool_kind == "ellipse" && fill_shape {
                        painter.add(egui::Shape::ellipse_filled(
                            bounds.center(),
                            Vec2::new(bounds.width() / 2.0, bounds.height() / 2.0),
                            preview_fill,
                        ));
                    }
                    if tool_kind == "rect" {
                        painter.rect_stroke(bounds, 0.0, preview_stroke, egui::StrokeKind::Middle);
                    } else if tool_kind == "round_rect" {
                        painter.rect_stroke(
                            bounds,
                            self.radius.max(0) as f32 * self.zoom,
                            preview_stroke,
                            egui::StrokeKind::Middle,
                        );
                    } else {
                        painter.circle_stroke(
                            bounds.center(),
                            bounds.width().min(bounds.height()) / 2.0,
                            preview_stroke,
                        );
                    }
                } else {
                    painter.circle_stroke(cursor, 6.0, preview_stroke);
                }
            }
            _ => {}
        }
    }

    fn tool_icon(ui: &mut egui::Ui, tool: &str, selected: bool) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
        let painter = ui.painter_at(rect);
        let visuals = ui.style().interact_selectable(&response, selected);
        painter.rect(
            rect,
            4.0,
            visuals.bg_fill,
            visuals.bg_stroke,
            egui::StrokeKind::Outside,
        );

        let center = rect.center();
        let icon_stroke = Stroke::new(1.8, visuals.fg_stroke.color);
        let left = rect.left() + 8.0;
        let right = rect.right() - 8.0;
        let top = rect.top() + 8.0;
        let bottom = rect.bottom() - 8.0;
        match tool {
            "select" => {
                painter.line_segment(
                    [
                        Pos2::new(left + 2.0, top),
                        Pos2::new(left + 2.0, bottom - 2.0),
                    ],
                    icon_stroke,
                );
                painter.line_segment(
                    [
                        Pos2::new(left + 2.0, top),
                        Pos2::new(right - 1.0, bottom - 2.0),
                    ],
                    icon_stroke,
                );
                painter.line_segment(
                    [
                        Pos2::new(left + 2.0, top),
                        Pos2::new(right - 1.0, top + 2.0),
                    ],
                    icon_stroke,
                );
            }
            "pixel" => {
                painter.circle_filled(center, 4.0, visuals.fg_stroke.color);
            }
            "line" => {
                painter.line_segment(
                    [Pos2::new(left, bottom), Pos2::new(right, top)],
                    icon_stroke,
                );
            }
            "polyline" => {
                painter.line_segment(
                    [Pos2::new(left, bottom), Pos2::new(center.x, top + 2.0)],
                    icon_stroke,
                );
                painter.line_segment(
                    [
                        Pos2::new(center.x, top + 2.0),
                        Pos2::new(right, bottom - 3.0),
                    ],
                    icon_stroke,
                );
                painter.circle_filled(Pos2::new(left, bottom), 2.0, visuals.fg_stroke.color);
                painter.circle_filled(Pos2::new(center.x, top + 2.0), 2.0, visuals.fg_stroke.color);
                painter.circle_filled(Pos2::new(right, bottom - 3.0), 2.0, visuals.fg_stroke.color);
            }
            "polygon" => {
                let points = (0..5)
                    .map(|index| {
                        let angle = -std::f32::consts::FRAC_PI_2
                            + index as f32 * std::f32::consts::TAU / 5.0;
                        Pos2::new(center.x + angle.cos() * 9.0, center.y + angle.sin() * 9.0)
                    })
                    .collect::<Vec<_>>();
                painter.add(egui::Shape::closed_line(points, icon_stroke));
            }
            "rect" => {
                let bounds = Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, bottom));
                painter.rect_stroke(bounds, 0.0, icon_stroke, egui::StrokeKind::Inside);
            }
            "fill_rect" => {
                let bounds = Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, bottom));
                painter.rect_filled(bounds, 0.0, visuals.fg_stroke.color);
                painter.rect_stroke(bounds, 0.0, icon_stroke, egui::StrokeKind::Inside);
            }
            "round_rect" => {
                let bounds = Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, bottom));
                painter.rect_stroke(bounds, 4.0, icon_stroke, egui::StrokeKind::Inside);
            }
            "fill_round_rect" => {
                let bounds = Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, bottom));
                painter.rect_filled(bounds, 4.0, visuals.fg_stroke.color);
                painter.rect_stroke(bounds, 4.0, icon_stroke, egui::StrokeKind::Inside);
            }
            "ellipse" => {
                painter.add(egui::Shape::ellipse_stroke(
                    center,
                    Vec2::new(10.0, 7.0),
                    icon_stroke,
                ));
            }
            "fill_circle" => {
                painter.add(egui::Shape::ellipse_filled(
                    center,
                    Vec2::new(10.0, 7.0),
                    visuals.fg_stroke.color,
                ));
                painter.add(egui::Shape::ellipse_stroke(
                    center,
                    Vec2::new(10.0, 7.0),
                    icon_stroke,
                ));
            }
            "fill_polygon" => {
                let points = (0..5)
                    .map(|index| {
                        let angle = -std::f32::consts::FRAC_PI_2
                            + index as f32 * std::f32::consts::TAU / 5.0;
                        Pos2::new(center.x + angle.cos() * 9.0, center.y + angle.sin() * 9.0)
                    })
                    .collect::<Vec<_>>();
                painter.add(egui::Shape::convex_polygon(
                    points,
                    visuals.fg_stroke.color,
                    icon_stroke,
                ));
            }
            "path" => {
                let points = (0..=20)
                    .map(|index| {
                        let t = index as f32 / 20.0;
                        Pos2::new(
                            left + t * (right - left),
                            center.y + (t * std::f32::consts::TAU).sin() * 6.0,
                        )
                    })
                    .collect::<Vec<_>>();
                painter.add(egui::Shape::line(points, icon_stroke));
            }
            _ => {}
        }
        response.on_hover_text(match tool {
            "fill_rect" => "fill rect",
            "fill_round_rect" => "fill round rect",
            "fill_circle" => "fill circle",
            "fill_polygon" => "fill polygon",
            _ => tool,
        })
    }

    fn color_icon(ui: &mut egui::Ui, color: &str, selected: bool) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
        let painter = ui.painter_at(rect);
        let visuals = ui.style().interact_selectable(&response, selected);
        painter.rect(
            rect,
            4.0,
            visuals.bg_fill,
            visuals.bg_stroke,
            egui::StrokeKind::Outside,
        );
        let center = rect.center();
        let radius = 9.0;
        match color {
            "black" => {
                painter.circle_filled(center, radius, Color32::BLACK);
            }
            "white" => {
                painter.circle_filled(center, radius, Color32::WHITE);
                painter.circle_stroke(center, radius, Stroke::new(1.0, Color32::DARK_GRAY));
            }
            "clear" => {
                let swatch = Rect::from_center_size(center, Vec2::splat(radius * 2.0));
                let half = swatch.width() / 2.0;
                for row in 0..2 {
                    for column in 0..2 {
                        let cell = Rect::from_min_size(
                            Pos2::new(
                                swatch.left() + column as f32 * half,
                                swatch.top() + row as f32 * half,
                            ),
                            Vec2::splat(half),
                        );
                        painter.rect_filled(
                            cell,
                            0.0,
                            if (row + column) % 2 == 0 {
                                config::colors::dither_swatch_checker_dark()
                            } else {
                                Color32::WHITE
                            },
                        );
                    }
                }
                painter.circle_stroke(center, radius, Stroke::new(1.0, Color32::DARK_GRAY));
                painter.line_segment(
                    [swatch.left_top(), swatch.right_bottom()],
                    Stroke::new(1.5, config::colors::dither_swatch_diagonal()),
                );
            }
            _ => {}
        }
        response.on_hover_text(color)
    }

    fn vector_row_icon(ui: &mut egui::Ui, object: &VectorObject, selected: bool) {
        let size = Vec2::splat(18.0);
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 3.0, ui.visuals().faint_bg_color);
        if selected {
            painter.rect_stroke(
                rect,
                3.0,
                ui.visuals().selection.stroke,
                egui::StrokeKind::Inside,
            );
        }

        if object.points.is_empty() {
            return;
        }

        let preview = rect.shrink(3.0);
        if preview.width() <= 0.0 || preview.height() <= 0.0 {
            return;
        }

        let (source_min_x, source_max_x, source_min_y, source_max_y) =
            if matches!(object.kind.as_str(), "rect" | "round_rect" | "ellipse")
                && object.points.len() >= 2
            {
                (
                    object.points[0][0].min(object.points[1][0]),
                    object.points[0][0].max(object.points[1][0]),
                    object.points[0][1].min(object.points[1][1]),
                    object.points[0][1].max(object.points[1][1]),
                )
            } else {
                (
                    object
                        .points
                        .iter()
                        .map(|point| point[0])
                        .fold(f32::INFINITY, f32::min),
                    object
                        .points
                        .iter()
                        .map(|point| point[0])
                        .fold(f32::NEG_INFINITY, f32::max),
                    object
                        .points
                        .iter()
                        .map(|point| point[1])
                        .fold(f32::INFINITY, f32::min),
                    object
                        .points
                        .iter()
                        .map(|point| point[1])
                        .fold(f32::NEG_INFINITY, f32::max),
                )
            };

        let source_w = (source_max_x - source_min_x).max(1.0);
        let source_h = (source_max_y - source_min_y).max(1.0);
        let scale = (preview.width() / source_w)
            .min(preview.height() / source_h)
            .max(0.001);
        let mapped_w = source_w * scale;
        let mapped_h = source_h * scale;
        let origin_x = preview.left() + (preview.width() - mapped_w) * 0.5;
        let origin_y = preview.top() + (preview.height() - mapped_h) * 0.5;

        let map = |point: [f32; 2]| {
            Pos2::new(
                origin_x + (point[0] - source_min_x) * scale,
                origin_y + (point[1] - source_min_y) * scale,
            )
        };

        let points: Vec<Pos2> = object.points.iter().copied().map(map).collect();
        let (draw_color, background_color) = match object.style.color.as_str() {
            "white" => (Color32::WHITE, Color32::BLACK),
            "clear" => (config::colors::clear_color(), Color32::BLACK),
            _ => (Color32::BLACK, Color32::WHITE),
        };
        painter.rect_filled(preview, 1.0, background_color);
        painter.rect_stroke(
            preview,
            1.0,
            Stroke::new(0.7, Color32::GRAY),
            egui::StrokeKind::Inside,
        );
        let stroke_color = draw_color;
        let stroke = Stroke::new(
            (object.style.width.max(1) as f32 * 0.45).clamp(0.9, 1.8),
            stroke_color,
        );

        match object.kind.as_str() {
            "pixel" => {
                painter.circle_filled(points[0], 1.8, draw_color);
            }
            "rect" if points.len() >= 2 => {
                let bounds = Rect::from_two_pos(points[0], points[1]);
                if object.style.fill {
                    painter.rect_filled(bounds, 0.0, draw_color);
                }
                painter.rect_stroke(bounds, 0.0, stroke, egui::StrokeKind::Inside);
            }
            "round_rect" if points.len() >= 2 => {
                let bounds = Rect::from_two_pos(points[0], points[1]);
                let radius = (object.style.radius.max(0) as f32 * scale)
                    .clamp(0.0, bounds.width().min(bounds.height()) * 0.5);
                if object.style.fill {
                    painter.rect_filled(bounds, radius, draw_color);
                }
                painter.rect_stroke(bounds, radius, stroke, egui::StrokeKind::Inside);
            }
            "ellipse" if points.len() >= 2 => {
                let bounds = Rect::from_two_pos(points[0], points[1]);
                let center = bounds.center();
                let radii = Vec2::new(bounds.width() * 0.5, bounds.height() * 0.5);
                if object.style.fill {
                    painter.add(egui::Shape::ellipse_filled(center, radii, draw_color));
                }
                painter.add(egui::Shape::ellipse_stroke(center, radii, stroke));
            }
            "polygon" if points.len() >= 3 => {
                if object.style.fill {
                    painter.add(egui::Shape::convex_polygon(
                        points.clone(),
                        draw_color,
                        stroke,
                    ));
                } else {
                    painter.add(egui::Shape::closed_line(points.clone(), stroke));
                }
                painter.add(egui::Shape::closed_line(points, stroke));
            }
            _ if points.len() >= 2 => {
                for pair in points.windows(2) {
                    painter.line_segment([pair[0], pair[1]], stroke);
                }
                if object.closed && points.len() > 2 {
                    painter.line_segment([*points.last().unwrap(), points[0]], stroke);
                }
            }
            _ => {
                painter.circle_stroke(preview.center(), 1.8, stroke);
            }
        }
    }

    fn vector_visibility_button(ui: &mut egui::Ui, visible: bool) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(
                config::ui::VECTOR_VISIBILITY_WIDTH,
                config::ui::VECTOR_ROW_HEIGHT,
            ),
            Sense::click(),
        );
        let painter = ui.painter_at(rect);
        let center = rect.center();
        let half_width = 8.0;
        let half_height = 5.0;
        let left = Pos2::new(center.x - half_width, center.y);
        let right = Pos2::new(center.x + half_width, center.y);
        let top = Pos2::new(center.x, center.y - half_height);
        let bottom = Pos2::new(center.x, center.y + half_height);
        let stroke = Stroke::new(1.5, ui.visuals().text_color());

        if visible {
            painter.line_segment([left, top], stroke);
            painter.line_segment([top, right], stroke);
            painter.line_segment([right, bottom], stroke);
            painter.line_segment([bottom, left], stroke);
            painter.circle_filled(center, 2.5, ui.visuals().text_color());
        } else {
            painter.line_segment([left, right], stroke);
            painter.line_segment(
                [
                    Pos2::new(left.x + 1.0, left.y + 6.0),
                    Pos2::new(right.x - 1.0, right.y - 6.0),
                ],
                stroke,
            );
        }

        response.on_hover_text(if visible { "Visible" } else { "Hidden" })
    }

    fn dither_icon(
        ui: &mut egui::Ui,
        pattern: &str,
        selected: bool,
        texture: Option<&egui::TextureHandle>,
    ) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
        let painter = ui.painter_at(rect);
        let visuals = ui.style().interact_selectable(&response, selected);
        let frame_stroke = if selected {
            Stroke::new(1.8, config::colors::dither_selected_border())
        } else {
            visuals.bg_stroke
        };
        painter.rect(
            rect,
            4.0,
            visuals.bg_fill,
            frame_stroke,
            egui::StrokeKind::Outside,
        );

        let sample = rect.shrink(7.0);
        painter.rect_filled(sample, 1.0, Color32::WHITE);
        painter.rect_stroke(
            sample,
            1.0,
            Stroke::new(0.8, Color32::GRAY),
            egui::StrokeKind::Inside,
        );
        if let Some(texture) = texture {
            painter.image(
                texture.id(),
                sample,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
            return response.on_hover_text(pattern);
        }
        let ink = visuals.fg_stroke.color;
        let dot = |x: f32, y: f32| {
            painter.circle_filled(
                Pos2::new(
                    sample.left() + x * sample.width(),
                    sample.top() + y * sample.height(),
                ),
                1.5,
                ink,
            );
        };
        let line = |a: (f32, f32), b: (f32, f32)| {
            painter.line_segment(
                [
                    Pos2::new(
                        sample.left() + a.0 * sample.width(),
                        sample.top() + a.1 * sample.height(),
                    ),
                    Pos2::new(
                        sample.left() + b.0 * sample.width(),
                        sample.top() + b.1 * sample.height(),
                    ),
                ],
                Stroke::new(1.3, ink),
            );
        };
        match pattern {
            "diagonal_line" => line((0.05, 0.95), (0.95, 0.05)),
            "vertical_line" => {
                line((0.25, 0.05), (0.25, 0.95));
                line((0.75, 0.05), (0.75, 0.95));
            }
            "horizontal_line" => {
                line((0.05, 0.25), (0.95, 0.25));
                line((0.05, 0.75), (0.95, 0.75));
            }
            "screen" => {
                for (x, y) in [(0.25, 0.25), (0.75, 0.75)] {
                    dot(x, y);
                }
            }
            "bayer_2x2" => {
                dot(0.25, 0.25);
            }
            "bayer_4x4" => {
                for (x, y) in [
                    (0.125, 0.125),
                    (0.625, 0.125),
                    (0.375, 0.375),
                    (0.875, 0.375),
                ] {
                    dot(x, y);
                }
                for (x, y) in [
                    (0.125, 0.625),
                    (0.625, 0.625),
                    (0.375, 0.875),
                    (0.875, 0.875),
                ] {
                    dot(x, y);
                }
            }
            "bayer_8x8" => {
                for (x, y) in [
                    (0.0625, 0.0625),
                    (0.3125, 0.1875),
                    (0.5625, 0.0625),
                    (0.8125, 0.1875),
                    (0.1875, 0.4375),
                    (0.4375, 0.5625),
                    (0.6875, 0.4375),
                    (0.9375, 0.5625),
                    (0.0625, 0.8125),
                    (0.3125, 0.9375),
                    (0.5625, 0.8125),
                    (0.8125, 0.9375),
                ] {
                    dot(x, y);
                }
            }
            "floyd_steinberg" => {
                for (x, y) in [
                    (0.2, 0.2),
                    (0.5, 0.35),
                    (0.8, 0.2),
                    (0.35, 0.75),
                    (0.7, 0.65),
                ] {
                    dot(x, y);
                }
            }
            "burkes" => {
                for (x, y) in [
                    (0.15, 0.2),
                    (0.4, 0.2),
                    (0.75, 0.3),
                    (0.25, 0.7),
                    (0.6, 0.8),
                ] {
                    dot(x, y);
                }
            }
            "atkinson" => {
                for (x, y) in [
                    (0.25, 0.2),
                    (0.6, 0.2),
                    (0.4, 0.5),
                    (0.75, 0.65),
                    (0.2, 0.8),
                ] {
                    dot(x, y);
                }
            }
            _ => {}
        }
        response.on_hover_text(pattern)
    }

    fn apply_raster_blend_color(current: Color32, color: Color32, blend: &str) -> Color32 {
        if color == Color32::TRANSPARENT {
            return Color32::TRANSPARENT;
        }
        if blend == "xor" {
            if current == Color32::BLACK {
                Color32::WHITE
            } else {
                Color32::BLACK
            }
        } else {
            color
        }
    }

    fn put_raster_pixel(
        pixels: &mut [Color32],
        width: usize,
        height: usize,
        x: i32,
        y: i32,
        color: Color32,
        blend: &str,
        line_width: f32,
        dither_pattern: &str,
    ) {
        let brush_radius = ((line_width.ceil() - 1.0) / 2.0).floor() as i32;
        for dy in -brush_radius..=brush_radius {
            for dx in -brush_radius..=brush_radius {
                if dx * dx + dy * dy > brush_radius * brush_radius {
                    continue;
                }
                let px = x + dx;
                let py = y + dy;
                if px >= 0 && py >= 0 && (px as usize) < width && (py as usize) < height {
                    if Self::dither_allows_pixel(dither_pattern, px as usize, py as usize) {
                        let index = py as usize * width + px as usize;
                        pixels[index] = Self::apply_raster_blend_color(pixels[index], color, blend);
                    }
                }
            }
        }
    }

    fn bayer_4x4_threshold(x: usize, y: usize) -> u8 {
        const MATRIX: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
        MATRIX[y % 4][x % 4]
    }

    fn bayer_8x8_threshold(x: usize, y: usize) -> u8 {
        const MATRIX: [[u8; 8]; 8] = [
            [0, 48, 12, 60, 3, 51, 15, 63],
            [32, 16, 44, 28, 35, 19, 47, 31],
            [8, 56, 4, 52, 11, 59, 7, 55],
            [40, 24, 36, 20, 43, 27, 39, 23],
            [2, 50, 14, 62, 1, 49, 13, 61],
            [34, 18, 46, 30, 33, 17, 45, 29],
            [10, 58, 6, 54, 9, 57, 5, 53],
            [42, 26, 38, 22, 41, 25, 37, 21],
        ];
        MATRIX[y % 8][x % 8]
    }

    fn dither_allows_pixel(pattern: &str, x: usize, y: usize) -> bool {
        match pattern {
            "none" => true,
            "diagonal_line" => (x + y) % 6 < 3,
            "vertical_line" => x % 4 < 2,
            "horizontal_line" => y % 4 < 2,
            "screen" => (x + y) % 2 == 0,
            "bayer_2x2" => (x % 2) == (y % 2),
            "bayer_4x4" => Self::bayer_4x4_threshold(x, y) < 8,
            "bayer_8x8" => Self::bayer_8x8_threshold(x, y) < 32,
            // These approximate Playdate's error-diffusion styles with stable 50% masks.
            "floyd_steinberg" => ((x * 5 + y * 3) & 7) < 4,
            "burkes" => ((x * 3 + y * 5 + x / 2) & 7) < 4,
            "atkinson" => ((x * 7 + y * 2 + (x ^ y)) & 7) < 4,
            _ => true,
        }
    }

    fn draw_raster_line(
        pixels: &mut [Color32],
        width: usize,
        height: usize,
        start: Pos2,
        end: Pos2,
        color: Color32,
        blend: &str,
        line_width: f32,
        dither_pattern: &str,
    ) {
        let mut x0 = start.x.round() as i32;
        let mut y0 = start.y.round() as i32;
        let x1 = end.x.round() as i32;
        let y1 = end.y.round() as i32;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut error = dx + dy;

        loop {
            Self::put_raster_pixel(
                pixels,
                width,
                height,
                x0,
                y0,
                color,
                blend,
                line_width,
                dither_pattern,
            );
            if x0 == x1 && y0 == y1 {
                break;
            }
            let twice_error = 2 * error;
            if twice_error >= dy {
                error += dy;
                x0 += sx;
            }
            if twice_error <= dx {
                error += dx;
                y0 += sy;
            }
        }
    }

    fn draw_raster_ellipse(
        pixels: &mut [Color32],
        width: usize,
        height: usize,
        bounds: [f32; 4],
        color: Color32,
        blend: &str,
        line_width: f32,
        dither_pattern: &str,
    ) {
        let left = bounds[0].round() as i32;
        let top = bounds[1].round() as i32;
        let right = bounds[2].round() as i32;
        let bottom = bounds[3].round() as i32;
        let rx = ((right - left).abs() / 2).max(1);
        let ry = ((bottom - top).abs() / 2).max(1);
        let center_x = (left + right) / 2;
        let center_y = (top + bottom) / 2;
        let rx_squared = rx * rx;
        let ry_squared = ry * ry;
        let mut x = 0_i32;
        let mut y = ry;
        let mut decision = ry_squared - rx_squared * ry + rx_squared / 4;

        while 2 * ry_squared * x <= 2 * rx_squared * y {
            for (px, py) in [
                (center_x + x, center_y + y),
                (center_x - x, center_y + y),
                (center_x + x, center_y - y),
                (center_x - x, center_y - y),
            ] {
                Self::put_raster_pixel(
                    pixels,
                    width,
                    height,
                    px,
                    py,
                    color,
                    blend,
                    line_width,
                    dither_pattern,
                );
            }
            if decision < 0 {
                x += 1;
                decision += 2 * ry_squared * x + ry_squared;
            } else {
                x += 1;
                y -= 1;
                decision += 2 * ry_squared * x - 2 * rx_squared * y + ry_squared;
            }
        }

        decision =
            ry_squared * (x * x + x) + rx_squared * (y - 1) * (y - 1) - rx_squared * ry_squared;
        while y >= 0 {
            for (px, py) in [
                (center_x + x, center_y + y),
                (center_x - x, center_y + y),
                (center_x + x, center_y - y),
                (center_x - x, center_y - y),
            ] {
                Self::put_raster_pixel(
                    pixels,
                    width,
                    height,
                    px,
                    py,
                    color,
                    blend,
                    line_width,
                    dither_pattern,
                );
            }
            if decision > 0 {
                y -= 1;
                decision -= 2 * rx_squared * y + rx_squared;
            } else {
                y -= 1;
                x += 1;
                decision += 2 * ry_squared * x - 2 * rx_squared * y + rx_squared;
            }
        }
    }

    fn rasterize_object(
        &self,
        pixels: &mut [Color32],
        width: usize,
        height: usize,
        object: &VectorObject,
        transparent_background: bool,
    ) {
        if !object.visible || object.points.is_empty() {
            return;
        }
        let color = match object.style.color.as_str() {
            "white" => Color32::WHITE,
            "clear" => {
                if transparent_background {
                    Color32::TRANSPARENT
                } else {
                    Color32::WHITE
                }
            }
            _ => Color32::BLACK,
        };
        let stroke_width = object.style.width.max(1) as f32;
        let points: Vec<Pos2> = object
            .points
            .iter()
            .map(|point| Pos2::new(point[0], point[1]))
            .collect();
        if matches!(object.kind.as_str(), "line" | "polyline" | "path") && points.len() >= 2 {
            for pair in points.windows(2) {
                Self::draw_raster_line(
                    pixels,
                    width,
                    height,
                    pair[0],
                    pair[1],
                    color,
                    &object.style.blend,
                    stroke_width,
                    &object.style.dither_pattern,
                );
            }
            if object.closed && points.len() > 2 {
                Self::draw_raster_line(
                    pixels,
                    width,
                    height,
                    *points.last().unwrap(),
                    points[0],
                    color,
                    &object.style.blend,
                    stroke_width,
                    &object.style.dither_pattern,
                );
            }
            for child in &object.children {
                self.rasterize_object(pixels, width, height, child, transparent_background);
            }
            return;
        }
        if object.kind == "ellipse" && !object.style.fill && points.len() >= 2 {
            Self::draw_raster_ellipse(
                pixels,
                width,
                height,
                [points[0].x, points[0].y, points[1].x, points[1].y],
                color,
                &object.style.blend,
                stroke_width,
                &object.style.dither_pattern,
            );
            for child in &object.children {
                self.rasterize_object(pixels, width, height, child, transparent_background);
            }
            return;
        }
        let min_x = points
            .iter()
            .map(|point| point.x)
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(0.0) as usize;
        let max_x = points
            .iter()
            .map(|point| point.x)
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil()
            .min(width as f32 - 1.0) as usize;
        let min_y = points
            .iter()
            .map(|point| point.y)
            .fold(f32::INFINITY, f32::min)
            .floor()
            .max(0.0) as usize;
        let max_y = points
            .iter()
            .map(|point| point.y)
            .fold(f32::NEG_INFINITY, f32::max)
            .ceil()
            .min(height as f32 - 1.0) as usize;
        if min_x > max_x || min_y > max_y {
            return;
        }

        if object.kind == "polygon" && points.len() >= 3 {
            for (start, end) in points
                .iter()
                .zip(points.iter().cycle().skip(1))
                .take(points.len())
            {
                Self::draw_raster_line(
                    pixels,
                    width,
                    height,
                    *start,
                    *end,
                    color,
                    &object.style.blend,
                    stroke_width,
                    &object.style.dither_pattern,
                );
            }
            if !object.style.fill {
                for child in &object.children {
                    self.rasterize_object(pixels, width, height, child, transparent_background);
                }
                return;
            }
        }

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let sample = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
                let drawn = match object.kind.as_str() {
                    "pixel" => sample.distance(points[0]) <= stroke_width,
                    "rect" if points.len() >= 2 => {
                        let left = points[0].x.min(points[1].x);
                        let right = points[0].x.max(points[1].x);
                        let top = points[0].y.min(points[1].y);
                        let bottom = points[0].y.max(points[1].y);
                        let inside = sample.x >= left
                            && sample.x <= right
                            && sample.y >= top
                            && sample.y <= bottom;
                        let edge_distance = (sample.x - left)
                            .min(right - sample.x)
                            .min((sample.y - top).min(bottom - sample.y));
                        inside && (object.style.fill || edge_distance <= stroke_width / 2.0)
                    }
                    "round_rect" if points.len() >= 2 => {
                        let left = points[0].x.min(points[1].x);
                        let right = points[0].x.max(points[1].x);
                        let top = points[0].y.min(points[1].y);
                        let bottom = points[0].y.max(points[1].y);
                        let half_w = ((right - left) / 2.0).max(0.0);
                        let half_h = ((bottom - top) / 2.0).max(0.0);
                        let radius = object.style.radius.max(0) as f32;
                        let corner = radius.min(half_w.min(half_h));
                        let center_x = (left + right) / 2.0;
                        let center_y = (top + bottom) / 2.0;
                        let qx = (sample.x - center_x).abs() - (half_w - corner);
                        let qy = (sample.y - center_y).abs() - (half_h - corner);
                        let outside = qx.max(0.0).hypot(qy.max(0.0));
                        let inside = qx.max(qy).min(0.0);
                        let signed_distance = outside + inside - corner;
                        if object.style.fill {
                            signed_distance <= 0.0
                        } else {
                            signed_distance.abs() <= stroke_width / 2.0
                        }
                    }
                    "ellipse" if points.len() >= 2 => {
                        let left = points[0].x.min(points[1].x);
                        let right = points[0].x.max(points[1].x);
                        let top = points[0].y.min(points[1].y);
                        let bottom = points[0].y.max(points[1].y);
                        let rx = (right - left) / 2.0;
                        let ry = (bottom - top) / 2.0;
                        if rx <= 0.0 || ry <= 0.0 {
                            false
                        } else {
                            let dx = (sample.x - (left + right) / 2.0) / rx;
                            let dy = (sample.y - (top + bottom) / 2.0) / ry;
                            let value = dx * dx + dy * dy;
                            object.style.fill && value <= 1.0
                                || !object.style.fill
                                    && (value - 1.0).abs() * rx.min(ry) <= stroke_width / 2.0
                        }
                    }
                    "polygon" if points.len() >= 3 => {
                        let filled =
                            object.style.fill && editor::point_in_polygon(sample, &object.points);
                        let edge = points
                            .iter()
                            .zip(points.iter().cycle().skip(1))
                            .take(points.len())
                            .any(|(start, end)| {
                                editor::distance_to_segment(sample, *start, *end)
                                    <= stroke_width / 2.0
                            });
                        filled || edge
                    }
                    _ if points.len() >= 2 => {
                        points.windows(2).any(|pair| {
                            editor::distance_to_segment(sample, pair[0], pair[1])
                                <= stroke_width / 2.0
                        }) || (object.closed
                            && editor::distance_to_segment(
                                sample,
                                *points.last().unwrap(),
                                points[0],
                            ) <= stroke_width / 2.0)
                    }
                    _ => false,
                };
                if drawn {
                    if Self::dither_allows_pixel(&object.style.dither_pattern, x, y) {
                        let index = y * width + x;
                        pixels[index] = Self::apply_raster_blend_color(
                            pixels[index],
                            color,
                            &object.style.blend,
                        );
                    }
                }
            }
        }

        for child in &object.children {
            self.rasterize_object(pixels, width, height, child, transparent_background);
        }
    }

    fn pixel_preview_with_background(
        &self,
        background: Color32,
        transparent_background: bool,
    ) -> Vec<Color32> {
        let width = self.doc.target.width.max(1) as usize;
        let height = self.doc.target.height.max(1) as usize;
        let mut pixels = vec![background; width * height];
        for layer in &self.doc.layers {
            if layer.visible {
                for object in &layer.objects {
                    self.rasterize_object(
                        &mut pixels,
                        width,
                        height,
                        object,
                        transparent_background,
                    );
                }
            }
        }
        pixels
    }

    fn draw_transparency_checkerboard(painter: &egui::Painter, rect: Rect, cell_size: f32) {
        let cell_size = cell_size.max(2.0);
        let columns = (rect.width() / cell_size).ceil() as i32;
        let rows = (rect.height() / cell_size).ceil() as i32;
        painter.rect_filled(rect, 0.0, config::colors::transparency_checker_light());
        for row in 0..rows {
            for column in 0..columns {
                if (row + column) % 2 == 1 {
                    let cell = Rect::from_min_size(
                        Pos2::new(
                            rect.left() + column as f32 * cell_size,
                            rect.top() + row as f32 * cell_size,
                        ),
                        Vec2::splat(cell_size),
                    );
                    painter.rect_filled(cell, 0.0, config::colors::transparency_checker_dark());
                }
            }
        }
    }

    fn draw_canvas(&mut self, ui: &mut egui::Ui) {
        let size = ui.available_size();
        self.viewport_size = size;
        let target_size = (self.doc.target.width, self.doc.target.height);
        if self.last_fitted_target != Some(target_size) {
            self.fit_canvas_to_viewport();
        }
        let max_zoom = self.max_zoom();
        self.zoom = self.zoom.min(max_zoom);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let painter = ui.painter_at(rect).with_clip_rect(rect);
        painter.rect_filled(rect, 0.0, Color32::WHITE);
        let w = self.doc.target.width as f32 * self.zoom;
        let h = self.doc.target.height as f32 * self.zoom;
        let canvas_rect = Rect::from_min_size(rect.left_top() + self.pan, Vec2::new(w, h));
        Self::draw_transparency_checkerboard(&painter, canvas_rect, 8.0 * self.zoom);
        painter.rect_stroke(
            canvas_rect,
            0.0,
            Stroke::new(1.0, Color32::DARK_GRAY),
            egui::StrokeKind::Outside,
        );
        let grid_step: i32 = if self.zoom >= 4.0 {
            1
        } else if self.zoom >= 2.0 {
            5
        } else {
            20
        };
        let grid_spacing = grid_step as f32 * self.zoom;
        if grid_spacing >= 4.0 {
            for x in (0..=self.doc.target.width).step_by(grid_step as usize) {
                let sx = canvas_rect.left() + x as f32 * self.zoom;
                let is_major = x % 20 == 0;
                painter.line_segment(
                    [
                        Pos2::new(sx, canvas_rect.top()),
                        Pos2::new(sx, canvas_rect.bottom()),
                    ],
                    Stroke::new(
                        if is_major { 1.0 } else { 0.5 },
                        if is_major {
                            config::colors::grid_major()
                        } else {
                            config::colors::grid_minor()
                        },
                    ),
                );
            }
            for y in (0..=self.doc.target.height).step_by(grid_step as usize) {
                let sy = canvas_rect.top() + y as f32 * self.zoom;
                let is_major = y % 20 == 0;
                painter.line_segment(
                    [
                        Pos2::new(canvas_rect.left(), sy),
                        Pos2::new(canvas_rect.right(), sy),
                    ],
                    Stroke::new(
                        if is_major { 1.0 } else { 0.5 },
                        if is_major {
                            config::colors::grid_major()
                        } else {
                            config::colors::grid_minor()
                        },
                    ),
                );
            }
        }
        let hovered_control_point = if self.tool == "select" && response.hovered() {
            response
                .hover_pos()
                .and_then(|pos| self.hit_test_control_point(rect, pos))
        } else {
            None
        };
        if self.pixel_preview {
            let pixels = self.pixel_preview_with_background(Color32::WHITE, false);
            let width = self.doc.target.width.max(1) as usize;
            let mut mesh = egui::Mesh::default();
            for (index, color) in pixels.into_iter().enumerate() {
                let x = index % width;
                let y = index / width;
                let pixel_rect = Rect::from_min_size(
                    Pos2::new(
                        canvas_rect.left() + x as f32 * self.zoom,
                        canvas_rect.top() + y as f32 * self.zoom,
                    ),
                    Vec2::splat(self.zoom),
                );
                mesh.add_colored_rect(pixel_rect, color);
            }
            if !mesh.indices.is_empty() {
                painter.add(egui::Shape::mesh(mesh));
            }
        } else {
            for layer in &self.doc.layers {
                if layer.visible {
                    for (index, object) in layer.objects.iter().enumerate() {
                        self.draw_object(&painter, rect, object);
                        if self.selected == Some((self.current_layer, index))
                            && !object.points.is_empty()
                        {
                            for (point_index, point) in object.points.iter().enumerate() {
                                let is_dragging_point = response
                                    .dragged_by(egui::PointerButton::Primary)
                                    && self.selected_point == Some(point_index);
                                let is_hovered =
                                    hovered_control_point == Some(point_index) || is_dragging_point;
                                let radius = if is_hovered {
                                    config::interaction::CONTROL_POINT_HOVER_RADIUS
                                } else {
                                    config::interaction::CONTROL_POINT_RADIUS
                                };
                                if is_hovered {
                                    painter.circle_filled(
                                        self.doc_to_screen(rect, *point),
                                        radius,
                                        config::colors::selection_fill(),
                                    );
                                }
                                painter.circle_stroke(
                                    self.doc_to_screen(rect, *point),
                                    radius,
                                    Stroke::new(
                                        if is_hovered { 2.5 } else { 1.5 },
                                        config::colors::selection_stroke(),
                                    ),
                                );
                            }
                        }
                    }
                }
            }
        }
        if self.pixel_preview && self.zoom >= 1.0 {
            let pixel_grid = Stroke::new(0.5, config::colors::pixel_grid_stroke());
            for x in 0..=self.doc.target.width {
                let sx = canvas_rect.left() + x as f32 * self.zoom;
                painter.line_segment(
                    [
                        Pos2::new(sx, canvas_rect.top()),
                        Pos2::new(sx, canvas_rect.bottom()),
                    ],
                    pixel_grid,
                );
            }
            for y in 0..=self.doc.target.height {
                let sy = canvas_rect.top() + y as f32 * self.zoom;
                painter.line_segment(
                    [
                        Pos2::new(canvas_rect.left(), sy),
                        Pos2::new(canvas_rect.right(), sy),
                    ],
                    pixel_grid,
                );
            }
        }
        if self.pixel_preview {
            if let Some((layer_index, object_index)) = self.selected {
                if let Some(object) = self
                    .doc
                    .layers
                    .get(layer_index)
                    .and_then(|layer| layer.objects.get(object_index))
                {
                    for (point_index, point) in object.points.iter().enumerate() {
                        let is_dragging_point = response.dragged_by(egui::PointerButton::Primary)
                            && self.selected_point == Some(point_index);
                        let is_hovered =
                            hovered_control_point == Some(point_index) || is_dragging_point;
                        let radius = if is_hovered {
                            config::interaction::CONTROL_POINT_HOVER_RADIUS
                        } else {
                            config::interaction::CONTROL_POINT_RADIUS
                        };
                        if is_hovered {
                            painter.circle_filled(
                                self.doc_to_screen(rect, *point),
                                radius,
                                config::colors::selection_fill(),
                            );
                        }
                        painter.circle_stroke(
                            self.doc_to_screen(rect, *point),
                            radius,
                            Stroke::new(
                                if is_hovered { 2.5 } else { 1.5 },
                                config::colors::selection_stroke(),
                            ),
                        );
                    }
                }
            }
        }
        if self.pending.len() > 1 {
            let pts: Vec<Pos2> = self
                .pending
                .iter()
                .map(|p| self.doc_to_screen(rect, *p))
                .collect();
            for pair in pts.windows(2) {
                painter.line_segment(
                    [pair[0], pair[1]],
                    Stroke::new(2.0, config::colors::selection_stroke()),
                );
            }
        }
        if !self.pending.is_empty() {
            let guide_stroke = Stroke::new(1.0, config::colors::guide_stroke());
            for (index, point) in self.pending.iter().enumerate() {
                let screen_point = self.doc_to_screen(rect, *point);
                let radius = if index == 0 { 7.0 } else { 5.0 };
                painter.circle_filled(screen_point, radius, config::colors::guide_fill());
                painter.circle_stroke(screen_point, radius, guide_stroke);
            }
        }
        if response.hovered() {
            if let Some(cursor) = response.hover_pos() {
                self.draw_tool_preview(&painter, rect, cursor);
            }
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.0 {
                self.zoom = (self.zoom * (1.0 + scroll.signum() * 0.1)).clamp(0.25, max_zoom);
            }
        }
        let space_down = ui.input(|i| i.key_down(egui::Key::Space));
        if space_down && response.hovered() {
            let is_panning = response.dragged_by(egui::PointerButton::Primary)
                || response.dragged_by(egui::PointerButton::Middle);
            ui.output_mut(|output| {
                output.cursor_icon = if is_panning {
                    egui::CursorIcon::Grabbing
                } else {
                    egui::CursorIcon::Grab
                };
            });
        }
        if (space_down && response.dragged_by(egui::PointerButton::Primary))
            || response.dragged_by(egui::PointerButton::Middle)
        {
            self.pan += ui.input(|i| i.pointer.delta());
        }
        if self.tool == "select" && !space_down && response.dragged_by(egui::PointerButton::Primary)
        {
            if self.selected.is_some() {
                if response.drag_started() {
                    // Lock drag target from the initial press position so point drag
                    // consistently moves only that point.
                    let drag_start = ui
                        .input(|input| input.pointer.press_origin())
                        .or_else(|| response.interact_pointer_pos());
                    self.selected_point =
                        drag_start.and_then(|position| self.hit_test_control_point(rect, position));
                    self.save_history();
                }
                let delta = ui.input(|i| i.pointer.delta()) / self.zoom;
                if let Some(point_index) = self.selected_point {
                    self.move_selected_point(point_index, delta);
                } else {
                    self.move_selected(delta);
                }
            }
        } else if self.tool == "select" && !space_down && response.clicked() {
            self.selected = response
                .interact_pointer_pos()
                .and_then(|pos| self.hit_test(rect, pos))
                .map(|index| (self.current_layer, index));
            self.selected_point = None;
        } else if response.clicked() && !space_down {
            if let Some(pos) = response.interact_pointer_pos() {
                let p = self.snap(self.screen_to_doc(rect, pos));
                match self.tool.as_str() {
                    "pixel" => {
                        self.pending = vec![p];
                        self.commit_pending(false);
                    }
                    "line" => {
                        self.pending.push(p);
                        if self.pending.len() == 2 {
                            self.commit_pending(false);
                        }
                    }
                    "polygon" | "fill_polygon" => {
                        self.pending.push(p);
                    }
                    "polyline" | "path" | "rect" | "fill_rect" | "round_rect"
                    | "fill_round_rect" | "ellipse" | "fill_circle" => {
                        self.pending.push(p);
                        if matches!(
                            self.tool.as_str(),
                            "rect"
                                | "fill_rect"
                                | "round_rect"
                                | "fill_round_rect"
                                | "ellipse"
                                | "fill_circle"
                        ) && self.pending.len() == 2
                        {
                            self.commit_pending(false);
                        }
                    }
                    _ => {}
                }
            }
        }
        if response.secondary_clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.commit_pending(self.current_tool_kind() == "polygon");
        }
    }

    fn preview(&self, ui: &mut egui::Ui) {
        let scale = 1.0_f32;
        let size = Vec2::new(
            self.doc.target.width as f32 * scale,
            self.doc.target.height as f32 * scale,
        );
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let painter = ui.painter_at(rect).with_clip_rect(rect);
        painter.rect_filled(rect, 0.0, Color32::WHITE);
        let pixels = self.pixel_preview_with_background(Color32::WHITE, false);
        let width = self.doc.target.width.max(1) as usize;
        for (index, color) in pixels.into_iter().enumerate() {
            let x = index % width;
            let y = index / width;
            let pixel_rect = Rect::from_min_size(
                Pos2::new(
                    rect.left() + x as f32 * scale,
                    rect.top() + y as f32 * scale,
                ),
                Vec2::splat(scale),
            );
            painter.rect_filled(pixel_rect, 0.0, color);
        }
        painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, Color32::DARK_GRAY),
            egui::StrokeKind::Outside,
        );
    }
}

impl eframe::App for DotStrokeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        #[cfg(target_os = "macos")]
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
        let main_focused = ui.input(|input| input.viewport().focused.unwrap_or(false));
        if main_focused && !self.main_was_focused && self.reference_window {
            ui.ctx().send_viewport_cmd_to(
                egui::ViewportId::from_hash_of("reference_preview"),
                egui::ViewportCommand::Focus,
            );
        }
        self.main_was_focused = main_focused;
        let (undo_pressed, redo_pressed) = ui.input(|input| {
            let modifier = input.modifiers.ctrl || input.modifiers.command;
            (
                modifier && input.key_pressed(egui::Key::Z),
                modifier && input.key_pressed(egui::Key::Y),
            )
        });
        if undo_pressed {
            self.undo_document();
        }
        if redo_pressed {
            self.redo_document();
        }
        let paste_reference = ui.input(|input| {
            let modifier = input.modifiers.ctrl || input.modifiers.command;
            modifier && input.key_pressed(egui::Key::V)
        });
        if paste_reference {
            self.add_reference_clipboard(ui.ctx());
        }
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
        let (native_new, native_load, native_save, native_export_png) = self.native_menu.actions();
        let (
            shortcut_new,
            shortcut_load,
            shortcut_save,
            shortcut_export_png,
            shortcut_copy_playdate_lua,
        ) = ui.input(|input| {
            let modifier = input.modifiers.ctrl || input.modifiers.command;
            (
                modifier && input.key_pressed(egui::Key::N), // 新規作成.
                modifier && input.key_pressed(egui::Key::O), // JSON読み込み.
                modifier && input.key_pressed(egui::Key::S), // JSON保存.
                modifier && input.modifiers.shift && input.key_pressed(egui::Key::E), // PNGエクスポート.
                modifier && input.key_pressed(egui::Key::P), // Copy Playdate Lua.
            )
        });
        if native_new || shortcut_new {
            self.begin_new_document();
        }
        if native_load || shortcut_load {
            self.load_json_document();
        }
        if native_save || shortcut_save {
            self.save_json_document();
        }
        if native_export_png || shortcut_export_png {
            self.export_png();
        }
        if shortcut_copy_playdate_lua {
            ui.ctx().copy_text(self.playdate_lua(false));
            self.status = "Copied Playdate Lua".into();
        }
        egui::Panel::top("menu").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New    Cmd+N").clicked() {
                        self.begin_new_document();
                        ui.close();
                    }
                    if ui.button("Load JSON    Cmd+O").clicked() {
                        self.load_json_document();
                        ui.close();
                    }
                    if ui.button("Save JSON    Cmd+S").clicked() {
                        self.save_json_document();
                        ui.close();
                    }
                    if ui.button("Export PNG    Cmd+Shift+E").clicked() {
                        self.export_png();
                        ui.close();
                    }
                });
                if ui.button("Reference Preview").clicked() {
                    self.reference_window = true;
                }
                ui.menu_button("Resolution", |ui| {
                    ui.label(format!(
                        "Current: {} x {}",
                        self.doc.target.width, self.doc.target.height
                    ));
                    ui.separator();
                    if ui.button("32 x 32").clicked() {
                        self.save_history();
                        self.doc.target.width = 32;
                        self.doc.target.height = 32;
                        ui.close();
                    }
                    if ui.button("400 x 240").clicked() {
                        self.save_history();
                        self.doc.target.width = 400;
                        self.doc.target.height = 240;
                        ui.close();
                    }
                });
            });
        });
        egui::Panel::left("tools").resizable(false).show(ui, |ui| {
            ui.heading("DotStroke");
            ui.label("Tool");
            ui.horizontal_wrapped(|ui| {
                for tool in [
                    "select",
                    "pixel",
                    "line",
                    "polyline",
                    "polygon",
                    "fill_polygon",
                    "rect",
                    "fill_rect",
                    "round_rect",
                    "fill_round_rect",
                    "ellipse",
                    "fill_circle",
                    "path",
                ] {
                    if Self::tool_icon(ui, tool, self.tool == tool).clicked() {
                        self.tool = tool.into();
                    }
                }
            });
            ui.label(format!("Selected: {}", self.current_tool_label()));
            ui.separator();
            ui.label("Style");
            let selected_style = if self.tool == "select" {
                self.selected
                    .and_then(|(layer_index, object_index)| {
                        self.doc
                            .layers
                            .get(layer_index)
                            .and_then(|layer| layer.objects.get(object_index))
                    })
                    .map(|object| object.style.clone())
            } else {
                None
            };
            let selected_kind = if self.tool == "select" {
                self.selected
                    .and_then(|(layer_index, object_index)| {
                        self.doc
                            .layers
                            .get(layer_index)
                            .and_then(|layer| layer.objects.get(object_index))
                    })
                    .map(|object| object.kind.clone())
            } else {
                None
            };
            let mut color = selected_style
                .as_ref()
                .map(|style| style.color.clone())
                .unwrap_or_else(|| self.color.clone());
            let original_color = color.clone();
            ui.horizontal(|ui| {
                for color_name in ["black", "white", "clear"] {
                    if Self::color_icon(ui, color_name, color == color_name).clicked() {
                        color = color_name.into();
                    }
                }
            });
            let color_changed = color != original_color;
            if color_changed {
                self.color = color.clone();
                if let Some((layer_index, object_index)) = self.selected {
                    if self.tool == "select" {
                        self.save_history();
                        if let Some(object) = self
                            .doc
                            .layers
                            .get_mut(layer_index)
                            .and_then(|layer| layer.objects.get_mut(object_index))
                        {
                            object.style.color = color;
                            self.status = "Color changed".into();
                        }
                    }
                }
            }

            let mut blend = selected_style
                .as_ref()
                .map(|style| style.blend.clone())
                .unwrap_or_else(|| self.blend.clone());
            let original_blend = blend.clone();
            ui.horizontal(|ui| {
                ui.label("Blend");
                ui.selectable_value(&mut blend, "normal".into(), "Normal");
                ui.selectable_value(&mut blend, "xor".into(), "XOR");
            });
            let blend_changed = blend != original_blend;
            if blend_changed {
                self.blend = blend.clone();
                if self.tool == "select" {
                    if let Some((layer_index, object_index)) = self.selected {
                        self.save_history();
                        if let Some(object) = self
                            .doc
                            .layers
                            .get_mut(layer_index)
                            .and_then(|layer| layer.objects.get_mut(object_index))
                        {
                            object.style.blend = blend;
                            self.status = "Blend mode changed".into();
                        }
                    }
                }
            }

            let mut dither_pattern = selected_style
                .as_ref()
                .map(|style| style.dither_pattern.clone())
                .unwrap_or_else(|| self.dither_pattern.clone());
            let original_dither_pattern = dither_pattern.clone();
            ui.label("Dither pattern");
            ui.horizontal_wrapped(|ui| {
                for pattern in DITHER_PATTERNS {
                    if Self::dither_icon(
                        ui,
                        pattern,
                        dither_pattern == pattern,
                        self.dither_icons.get(pattern),
                    )
                    .clicked()
                    {
                        dither_pattern = pattern.into();
                    }
                }
            });
            let dither_changed = dither_pattern != original_dither_pattern;
            if dither_changed {
                self.dither_pattern = dither_pattern.clone();
                if self.tool == "select" {
                    if let Some((layer_index, object_index)) = self.selected {
                        self.save_history();
                        if let Some(object) = self
                            .doc
                            .layers
                            .get_mut(layer_index)
                            .and_then(|layer| layer.objects.get_mut(object_index))
                        {
                            object.style.dither_pattern = dither_pattern;
                            self.status = "Dither pattern changed".into();
                        }
                    }
                }
            }
            let mut width = selected_style
                .as_ref()
                .map_or(self.width, |style| style.width);
            let width_response = ui.add(egui::Slider::new(&mut width, 1..=8).text("Width"));
            if width_response.changed() {
                self.width = width;
                if self.tool == "select" {
                    if let Some((layer_index, object_index)) = self.selected {
                        self.save_history();
                        if let Some(object) = self
                            .doc
                            .layers
                            .get_mut(layer_index)
                            .and_then(|layer| layer.objects.get_mut(object_index))
                        {
                            object.style.width = width;
                            self.status = "Width changed".into();
                        }
                    }
                }
            } else if self.tool != "select" {
                self.width = width;
            }

            if matches!(self.tool.as_str(), "round_rect" | "fill_round_rect")
                || selected_style.as_ref().is_some()
                    && matches!(selected_kind.as_deref(), Some("round_rect"))
            {
                let mut radius = selected_style
                    .as_ref()
                    .map_or(self.radius, |style| style.radius);
                let radius_response =
                    ui.add(egui::Slider::new(&mut radius, 0..=16).text("Corner radius"));
                if radius_response.changed() {
                    self.radius = radius;
                    if self.tool == "select" {
                        if let Some((layer_index, object_index)) = self.selected {
                            self.save_history();
                            if let Some(object) = self
                                .doc
                                .layers
                                .get_mut(layer_index)
                                .and_then(|layer| layer.objects.get_mut(object_index))
                            {
                                object.style.radius = radius;
                                self.status = "Corner radius changed".into();
                            }
                        }
                    }
                } else if self.tool != "select" {
                    self.radius = radius;
                }
            }
            egui::ComboBox::from_id_salt("rounding")
                .selected_text(&self.rounding)
                .show_ui(ui, |ui| {
                    for mode in ["floor", "ceil", "nearest", "subpixel"] {
                        ui.selectable_value(&mut self.rounding, mode.into(), mode);
                    }
                });
            ui.separator();
            if ui.button("Finalize").clicked() {
                self.commit_pending(self.current_tool_kind() == "polygon");
            }
            if ui.button("Cancel").clicked() {
                self.pending.clear();
            }
            ui.separator();
            ui.separator();
            ui.label(&self.status);
            ui.label("Space + left-drag: pan");
            ui.label("Middle-drag: pan");
            ui.label("Wheel: zoom");
            ui.label("Right-click/Enter: finalize");
            if self.tool == "select" {
                ui.label("Hover control point: drag");
            }
        });
        egui::Panel::right("preview")
            .resizable(false)
            .default_size(config::ui::PREVIEW_PANEL_WIDTH)
            .show(ui, |ui| {
                ui.heading("1-bit Preview");
                self.preview(ui);
                if ui.button("Copy Playdate Lua").clicked() {
                    ui.ctx().copy_text(self.playdate_lua(false));
                    self.status = "Copied Playdate Lua".into();
                }
                if ui.button("Copy Anim Lua").clicked() {
                    ui.ctx().copy_text(self.playdate_lua(true));
                    self.status = "Copied Animation Lua".into();
                }
                ui.separator();
                ui.heading("Vectors");
                let mut reorder_request = None;
                let mut drag_target = None;
                let mut drag_stopped = false;
                let mut visibility_toggle = None;
                egui::ScrollArea::vertical()
                    .min_scrolled_height(config::ui::VECTOR_LIST_MIN_HEIGHT)
                    .max_height(config::ui::VECTOR_LIST_MIN_HEIGHT)
                    .show(ui, |ui| {
                        let vector_rows: Vec<(usize, String, VectorObject)> = self.doc.layers
                            [self.current_layer]
                            .objects
                            .iter()
                            .enumerate()
                            .map(|(index, object)| {
                                (
                                    index,
                                    format!("{}: {}", index + 1, object.kind),
                                    object.clone(),
                                )
                            })
                            .collect();
                        for (index, name, object) in vector_rows {
                            let is_selected = self.selected == Some((self.current_layer, index));
                            let mut clicked = false;
                            let row_response = ui
                                .horizontal(|ui| {
                                    let drag_response = ui.add_sized(
                                        [
                                            config::ui::VECTOR_DRAG_HANDLE_WIDTH,
                                            config::ui::VECTOR_ROW_HEIGHT,
                                        ],
                                        egui::Label::new("≡").selectable(false),
                                    );
                                    drag_response.clone().on_hover_text("Drag to reorder");
                                    if Self::vector_visibility_button(ui, object.visible).clicked()
                                    {
                                        visibility_toggle = Some(index);
                                    }
                                    Self::vector_row_icon(ui, &object, is_selected);
                                    if ui.selectable_label(is_selected, name).clicked() {
                                        clicked = true;
                                    }
                                    if ui.small_button("↑").on_hover_text("Move up").clicked()
                                        && index > 0
                                    {
                                        reorder_request = Some((index, index - 1));
                                    }
                                    let object_count =
                                        self.doc.layers[self.current_layer].objects.len();
                                    if ui.small_button("↓").on_hover_text("Move down").clicked()
                                        && index + 1 < object_count
                                    {
                                        reorder_request = Some((index, index + 1));
                                    }
                                })
                                .response;
                            let row_left = row_response.rect.left();
                            let row_top = row_response.rect.top();
                            let row_bottom = row_response.rect.bottom();
                            let handle_rect = egui::Rect::from_min_max(
                                egui::pos2(row_left, row_top),
                                egui::pos2(
                                    row_left + config::ui::VECTOR_DRAG_HANDLE_WIDTH,
                                    row_bottom,
                                ),
                            );
                            let content_rect = egui::Rect::from_min_max(
                                egui::pos2(
                                    row_left
                                        + config::ui::VECTOR_DRAG_HANDLE_WIDTH
                                        + config::ui::VECTOR_VISIBILITY_WIDTH,
                                    row_top,
                                ),
                                egui::pos2(
                                    (row_response.rect.right()
                                        - config::ui::VECTOR_ROW_ACTION_WIDTH)
                                        .max(row_left),
                                    row_bottom,
                                ),
                            );
                            let handle_drag_response = ui.interact(
                                handle_rect,
                                ui.id().with(("vector_handle_drag", index)),
                                egui::Sense::click_and_drag(),
                            );
                            let content_drag_response = ui.interact(
                                content_rect,
                                ui.id().with(("vector_content_drag", index)),
                                egui::Sense::click_and_drag(),
                            );
                            if handle_drag_response.drag_started()
                                || content_drag_response.drag_started()
                            {
                                self.dragging_vector = Some(index);
                                self.selected = Some((self.current_layer, index));
                                self.selected_point = None;
                                self.tool = "select".into();
                            }
                            if handle_drag_response.drag_stopped()
                                || content_drag_response.drag_stopped()
                            {
                                drag_stopped = true;
                            }
                            if self.dragging_vector.is_some()
                                && ui.input(|input| {
                                    input.pointer.latest_pos().is_some_and(|position| {
                                        row_response.rect.contains(position)
                                    })
                                })
                            {
                                drag_target = Some(index);
                            }
                            if clicked
                                || handle_drag_response.clicked()
                                || content_drag_response.clicked()
                            {
                                self.selected = Some((self.current_layer, index));
                                self.selected_point = None;
                                self.tool = "select".into();
                            }
                        }
                    });
                if drag_stopped || ui.input(|input| input.pointer.any_released()) {
                    if let (Some(from), Some(to)) = (self.dragging_vector.take(), drag_target) {
                        if from != to {
                            reorder_request = Some((from, to));
                        }
                    } else {
                        self.dragging_vector = None;
                    }
                }
                if let Some(object_index) = visibility_toggle {
                    self.save_history();
                    if let Some(object) = self
                        .doc
                        .layers
                        .get_mut(self.current_layer)
                        .and_then(|layer| layer.objects.get_mut(object_index))
                    {
                        object.visible = !object.visible;
                        self.status = if object.visible {
                            "Vector visible".into()
                        } else {
                            "Vector hidden".into()
                        };
                    }
                }
                if let Some((from, to)) = reorder_request {
                    self.reorder_vector(self.current_layer, from, to);
                }
                self.paint_vector_drag_preview(ui.ctx());
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        self.delete_selected();
                    }
                    if ui.button("Duplicate").clicked() {
                        self.duplicate_selected();
                    }
                });

                ui.separator();
                ui.heading("Control Points");
                let selected_points = self
                    .selected
                    .and_then(|(layer_index, object_index)| {
                        self.doc
                            .layers
                            .get(layer_index)
                            .and_then(|layer| layer.objects.get(object_index))
                    })
                    .map(|object| object.points.clone());

                let mut point_edits: Vec<(usize, [f32; 2])> = Vec::new();
                if let Some(points) = selected_points {
                    if points.is_empty() {
                        ui.label("制御点がありません");
                    } else {
                        for (index, point) in points.iter().enumerate() {
                            ui.horizontal(|ui| {
                                let is_selected_point = self.selected_point == Some(index);
                                if ui
                                    .selectable_label(is_selected_point, format!("P{}", index + 1))
                                    .clicked()
                                {
                                    self.selected_point = Some(index);
                                    self.tool = "select".into();
                                }

                                let mut x = point[0];
                                let mut y = point[1];
                                let x_changed = ui
                                    .add(egui::DragValue::new(&mut x).speed(0.1).prefix("x: "))
                                    .changed();
                                let y_changed = ui
                                    .add(egui::DragValue::new(&mut y).speed(0.1).prefix("y: "))
                                    .changed();
                                if x_changed || y_changed {
                                    point_edits.push((index, [x, y]));
                                }
                            });
                        }
                    }
                } else {
                    ui.label("ベクターを選択してください");
                }

                if !point_edits.is_empty() {
                    self.save_history();
                    if let Some((layer_index, object_index)) = self.selected {
                        if let Some(object) = self
                            .doc
                            .layers
                            .get_mut(layer_index)
                            .and_then(|layer| layer.objects.get_mut(object_index))
                        {
                            for (index, point) in point_edits {
                                if let Some(target) = object.points.get_mut(index) {
                                    *target = point;
                                }
                            }
                            self.status = "Control point updated".into();
                        }
                    }
                }

                let selected_fill_state = self
                    .selected
                    .and_then(|(layer_index, object_index)| {
                        self.doc
                            .layers
                            .get(layer_index)
                            .and_then(|layer| layer.objects.get(object_index))
                    })
                    .map(|object| (object.kind.clone(), object.style.fill));
                if let Some((kind, mut fill)) = selected_fill_state {
                    if matches!(kind.as_str(), "rect" | "round_rect" | "ellipse" | "polygon") {
                        if ui.checkbox(&mut fill, "Fill").changed() {
                            if let Some((layer_index, object_index)) = self.selected {
                                self.save_history();
                                if let Some(object) = self
                                    .doc
                                    .layers
                                    .get_mut(layer_index)
                                    .and_then(|layer| layer.objects.get_mut(object_index))
                                {
                                    object.style.fill = fill;
                                    self.status = if fill {
                                        "Fill enabled".into()
                                    } else {
                                        "Fill disabled".into()
                                    };
                                }
                            }
                        }
                    }
                }
            });
        egui::containers::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Undo").clicked() {
                    self.undo_document();
                }
                if ui.button("Redo").clicked() {
                    self.redo_document();
                }
                ui.separator();
                ui.label(format!("Zoom: {:.2}x", self.zoom));
                if ui.button("-").clicked() {
                    self.zoom = (self.zoom - 0.25).max(0.25);
                }
                if ui.button("+").clicked() {
                    self.zoom = (self.zoom + 0.25).min(self.max_zoom());
                }
                if ui.button("Reset").clicked() {
                    self.zoom = 2.0;
                    self.pan = Vec2::ZERO;
                }
                ui.separator();
                ui.checkbox(&mut self.pixel_preview, "Pixel preview");
            });
            ui.separator();
            self.draw_canvas(ui);
        });
        if self.new_dialog {
            let mut create_document = None;
            egui::Window::new("New Document")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label("Canvas resolution");
                    ui.horizontal(|ui| {
                        ui.label("Width");
                        ui.add(egui::TextEdit::singleline(&mut self.new_width).desired_width(70.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Height");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.new_height).desired_width(70.0),
                        );
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Create").clicked() {
                            let width = self.new_width.trim().parse::<i32>().ok();
                            let height = self.new_height.trim().parse::<i32>().ok();
                            if let (Some(width), Some(height)) = (width, height) {
                                if width > 0 && height > 0 {
                                    create_document = Some((width, height));
                                } else {
                                    self.status = "Resolution must be positive".into();
                                }
                            } else {
                                self.status = "Enter numeric width and height".into();
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.new_dialog = false;
                        }
                    });
                });
            if let Some((width, height)) = create_document {
                self.save_history();
                self.doc = Document::default();
                self.doc.target.width = width;
                self.doc.target.height = height;
                self.current_file = None;
                self.pending.clear();
                self.selected = None;
                self.selected_point = None;
                self.current_layer = 0;
                self.new_dialog = false;
                self.status = format!("New document: {} x {}", width, height);
            }
        }
        if self.reference_window {
            let viewport_id = egui::ViewportId::from_hash_of("reference_preview");
            ui.ctx().show_viewport_immediate(
                viewport_id,
                egui::ViewportBuilder::default()
                    .with_title("Reference Preview")
                    .with_inner_size([720.0, 620.0])
                    .with_min_inner_size([360.0, 260.0]),
                |ui, _class| {
                    if ui.input(|input| input.viewport().close_requested()) {
                        self.reference_window = false;
                    }
                    self.draw_reference_preview(ui);
                },
            );
        }
    }
}

fn configure_fonts(ctx: &egui::Context) {
    // egui's built-in font is intentionally small and does not contain all
    // arrows, Japanese characters, or other UI symbols. Add a system CJK font
    // as a fallback so these glyphs render instead of becoming tofu boxes.
    const SYSTEM_FONT_CANDIDATES: [&str; 8] = [
        "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
        "/System/Library/Fonts/AppleSDGothicNeo.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJKjp-Regular.otf",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "C:\\Windows\\Fonts\\meiryo.ttc",
        "C:\\Windows\\Fonts\\msgothic.ttc",
        "C:\\Windows\\Fonts\\YuGothR.ttc",
    ];

    let system_font = SYSTEM_FONT_CANDIDATES
        .iter()
        .find_map(|path| fs::read(path).ok().map(|bytes| (path.to_string(), bytes)));

    let mut fonts = egui::FontDefinitions::default();
    if let Some((font_name, font_bytes)) = system_font {
        fonts.font_data.insert(
            font_name.clone(),
            egui::FontData::from_owned(font_bytes).into(),
        );
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            if let Some(family_fonts) = fonts.families.get_mut(&family) {
                family_fonts.push(font_name.clone());
            }
        }
    }
    ctx.set_fonts(fonts);

    ctx.all_styles_mut(|style| {
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::proportional(config::ui::FONT_SIZE_BODY),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::proportional(config::ui::FONT_SIZE_BUTTON),
        );
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::proportional(config::ui::FONT_SIZE_HEADING),
        );
        style.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::proportional(config::ui::FONT_SIZE_SMALL),
        );
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::monospace(config::ui::FONT_SIZE_MONOSPACE),
        );
    });
}

fn main() -> eframe::Result {
    let native_menu = ui::NativeMenu::new();
    native_menu.init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("DotStroke for Playdate (egui)")
            .with_inner_size([1500.0, 760.0]),
        event_loop_builder: Some(Box::new(|builder| {
            #[cfg(target_os = "macos")]
            {
                use winit::platform::macos::EventLoopBuilderExtMacOS;
                builder.with_default_menu(false);
            }
        })),
        ..Default::default()
    };
    eframe::run_native(
        "DotStroke",
        options,
        Box::new(move |_cc| {
            configure_fonts(&_cc.egui_ctx);
            let mut app = DotStrokeApp::default();
            app.native_menu = native_menu;
            app.load_dither_icons(&_cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}
