use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
mod editor;
mod export;
mod io;
mod model;
mod render;
mod ui;

use editor::History;
use model::{Document, Style, VectorObject, DEFAULT_HEIGHT, DEFAULT_WIDTH};
use std::{
    collections::HashMap,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

const CONTROL_POINT_RADIUS: f32 = 4.0;
const CONTROL_POINT_HOVER_RADIUS: f32 = 7.0;
const CONTROL_POINT_HIT_RADIUS: f32 = 24.0;
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

struct DotStrokeApp {
    doc: Document,
    tool: String,
    color: String,
    width: i32,
    fill: bool,
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
    current_layer: usize,
    status: String,
    history: History,
    new_dialog: bool,
    new_width: String,
    new_height: String,
    current_file: Option<PathBuf>,
    native_menu: ui::NativeMenu,
    dither_icons: HashMap<String, egui::TextureHandle>,
}

impl Default for DotStrokeApp {
    fn default() -> Self {
        Self {
            doc: Document::default(),
            tool: "line".into(),
            color: "black".into(),
            width: 1,
            fill: false,
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
            current_layer: 0,
            status: "Ready".into(),
            history: History::default(),
            new_dialog: false,
            new_width: DEFAULT_WIDTH.to_string(),
            new_height: DEFAULT_HEIGHT.to_string(),
            current_file: None,
            native_menu: ui::NativeMenu::new(),
            dither_icons: HashMap::new(),
        }
    }
}

impl DotStrokeApp {
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
        let hit_radius = editor::hit_radius(rect, self.zoom).min(CONTROL_POINT_HIT_RADIUS);
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
            if let Some(mut copy) = copy {
                self.save_history();
                for point in &mut copy.points {
                    point[0] += 8.0;
                    point[1] += 8.0;
                }
                let new_index = object_index + 1;
                self.doc.layers[layer_index].objects.insert(new_index, copy);
                self.selected = Some((layer_index, new_index));
                self.selected_point = None;
                self.status = "Vector duplicated".into();
            }
        }
    }

    fn commit_pending(&mut self, closed: bool) {
        let required = match self.tool.as_str() {
            "polygon" => 3,
            _ => 2,
        };
        if self.pending.len() < required {
            return;
        }
        self.save_history();
        let layer = &mut self.doc.layers[self.current_layer];
        layer.objects.push(VectorObject {
            kind: self.tool.clone(),
            points: self.pending.drain(..).collect(),
            closed,
            style: Style {
                color: self.color.clone(),
                width: self.width,
                fill: self.fill,
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

    fn append_lua_object(output: &mut String, object: &VectorObject) {
        if !object.visible || object.points.is_empty() {
            return;
        }

        let point =
            |p: &[f32; 2]| format!("{}, {}", Self::lua_number(p[0]), Self::lua_number(p[1]));
        let points = |points: &[[f32; 2]]| points.iter().map(point).collect::<Vec<_>>().join(", ");
        let _ = writeln!(
            output,
            "gfx.setColor({})",
            export::lua_color(&object.style.color)
        );
        if let Some(pattern) = export::lua_dither_pattern(&object.style.dither_pattern) {
            let _ = writeln!(output, "gfx.setDitherPattern(0.5, {})", pattern);
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
            Self::append_lua_object(output, child);
        }
    }

    fn playdate_lua(&self) -> String {
        let mut output = String::from("local gfx <const> = playdate.graphics\n");
        for layer in &self.doc.layers {
            if layer.visible {
                for object in &layer.objects {
                    Self::append_lua_object(&mut output, object);
                }
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
            "white" => Color32::GRAY,
            "clear" => Color32::from_rgb(60, 150, 255),
            _ => Color32::BLACK,
        };
        let stroke = Stroke::new((object.style.width.max(1) as f32) * zoom, color);
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

    fn draw_tool_preview(&self, painter: &egui::Painter, rect: Rect, cursor: Pos2) {
        if self.tool == "select" {
            return;
        }

        let cursor = self.snap(self.screen_to_doc(rect, cursor));
        let cursor = self.doc_to_screen(rect, cursor);
        let preview_color = Color32::from_rgba_unmultiplied(255, 0, 100, 105);
        let preview_fill = Color32::from_rgba_unmultiplied(255, 0, 100, 20);
        let preview_stroke = Stroke::new(
            (self.width.max(1) as f32 * self.zoom).max(1.0),
            preview_color,
        );

        match self.tool.as_str() {
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
            "polygon" => {
                if let Some(point) = self.pending.last() {
                    painter
                        .line_segment([self.doc_to_screen(rect, *point), cursor], preview_stroke);
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
                    if matches!(self.tool.as_str(), "rect" | "round_rect") && self.fill {
                        let radius = if self.tool == "round_rect" {
                            self.radius.max(0) as f32 * self.zoom
                        } else {
                            0.0
                        };
                        painter.rect_filled(bounds, radius, preview_fill);
                    }
                    if self.tool == "ellipse" && self.fill {
                        painter.add(egui::Shape::ellipse_filled(
                            bounds.center(),
                            Vec2::new(bounds.width() / 2.0, bounds.height() / 2.0),
                            preview_fill,
                        ));
                    }
                    if self.tool == "rect" {
                        painter.rect_stroke(bounds, 0.0, preview_stroke, egui::StrokeKind::Middle);
                    } else if self.tool == "round_rect" {
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

    fn tool_icon(ui: &mut egui::Ui, tool: &str, selected: bool, filled: bool) -> egui::Response {
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
                if filled {
                    painter.add(egui::Shape::convex_polygon(
                        points,
                        visuals.fg_stroke.color,
                        icon_stroke,
                    ));
                } else {
                    painter.add(egui::Shape::closed_line(points, icon_stroke));
                }
            }
            "rect" => {
                let bounds = Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, bottom));
                if filled {
                    painter.rect_filled(bounds, 0.0, visuals.fg_stroke.color);
                }
                painter.rect_stroke(bounds, 0.0, icon_stroke, egui::StrokeKind::Inside);
            }
            "round_rect" => {
                let bounds = Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, bottom));
                if filled {
                    painter.rect_filled(bounds, 4.0, visuals.fg_stroke.color);
                }
                painter.rect_stroke(bounds, 4.0, icon_stroke, egui::StrokeKind::Inside);
            }
            "ellipse" => {
                if filled {
                    painter.add(egui::Shape::ellipse_filled(
                        center,
                        Vec2::new(10.0, 7.0),
                        visuals.fg_stroke.color,
                    ));
                }
                painter.add(egui::Shape::ellipse_stroke(
                    center,
                    Vec2::new(10.0, 7.0),
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
        response.on_hover_text(tool)
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
        let swatch = rect.shrink(8.0);
        match color {
            "black" => {
                painter.rect_filled(swatch, 2.0, Color32::BLACK);
            }
            "white" => {
                painter.rect_filled(swatch, 2.0, Color32::WHITE);
                painter.rect_stroke(
                    swatch,
                    2.0,
                    Stroke::new(1.0, Color32::DARK_GRAY),
                    egui::StrokeKind::Inside,
                );
            }
            "clear" => {
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
                                Color32::from_gray(215)
                            } else {
                                Color32::WHITE
                            },
                        );
                    }
                }
                painter.line_segment(
                    [swatch.left_top(), swatch.right_bottom()],
                    Stroke::new(1.5, Color32::from_rgb(60, 150, 255)),
                );
                painter.rect_stroke(
                    swatch,
                    2.0,
                    Stroke::new(1.0, Color32::DARK_GRAY),
                    egui::StrokeKind::Inside,
                );
            }
            _ => {}
        }
        response.on_hover_text(color)
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
        painter.rect(
            rect,
            4.0,
            visuals.bg_fill,
            visuals.bg_stroke,
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

    fn put_raster_pixel(
        pixels: &mut [Color32],
        width: usize,
        height: usize,
        x: i32,
        y: i32,
        color: Color32,
        line_width: f32,
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
                    pixels[py as usize * width + px as usize] = color;
                }
            }
        }
    }

    fn draw_raster_line(
        pixels: &mut [Color32],
        width: usize,
        height: usize,
        start: Pos2,
        end: Pos2,
        color: Color32,
        line_width: f32,
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
            Self::put_raster_pixel(pixels, width, height, x0, y0, color, line_width);
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
        line_width: f32,
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
                Self::put_raster_pixel(pixels, width, height, px, py, color, line_width);
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
                Self::put_raster_pixel(pixels, width, height, px, py, color, line_width);
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
    ) {
        if !object.visible || object.points.is_empty() {
            return;
        }
        let color = match object.style.color.as_str() {
            "white" => Color32::WHITE,
            "clear" => Color32::WHITE,
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
                    stroke_width,
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
                    stroke_width,
                );
            }
            for child in &object.children {
                self.rasterize_object(pixels, width, height, child);
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
                stroke_width,
            );
            for child in &object.children {
                self.rasterize_object(pixels, width, height, child);
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
                Self::draw_raster_line(pixels, width, height, *start, *end, color, stroke_width);
            }
            if !object.style.fill {
                for child in &object.children {
                    self.rasterize_object(pixels, width, height, child);
                }
                return;
            }
        }

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let sample = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
                let drawn = match object.kind.as_str() {
                    "pixel" => sample.distance(points[0]) <= stroke_width,
                    "rect" | "round_rect" if points.len() >= 2 => {
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
                    pixels[y * width + x] = color;
                }
            }
        }

        for child in &object.children {
            self.rasterize_object(pixels, width, height, child);
        }
    }

    fn pixel_preview(&self) -> Vec<Color32> {
        let width = self.doc.target.width.max(1) as usize;
        let height = self.doc.target.height.max(1) as usize;
        let mut pixels = vec![Color32::WHITE; width * height];
        for layer in &self.doc.layers {
            if layer.visible {
                for object in &layer.objects {
                    self.rasterize_object(&mut pixels, width, height, object);
                }
            }
        }
        pixels
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
                            Color32::from_gray(205)
                        } else {
                            Color32::from_gray(232)
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
                            Color32::from_gray(205)
                        } else {
                            Color32::from_gray(232)
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
            let pixels = self.pixel_preview();
            let width = self.doc.target.width.max(1) as usize;
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
                painter.rect_filled(pixel_rect, 0.0, color);
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
                                    CONTROL_POINT_HOVER_RADIUS
                                } else {
                                    CONTROL_POINT_RADIUS
                                };
                                if is_hovered {
                                    painter.circle_filled(
                                        self.doc_to_screen(rect, *point),
                                        radius,
                                        Color32::from_rgba_unmultiplied(255, 0, 100, 70),
                                    );
                                }
                                painter.circle_stroke(
                                    self.doc_to_screen(rect, *point),
                                    radius,
                                    Stroke::new(
                                        if is_hovered { 2.5 } else { 1.5 },
                                        Color32::from_rgb(255, 0, 100),
                                    ),
                                );
                            }
                        }
                    }
                }
            }
        }
        if self.pixel_preview && self.zoom >= 1.0 {
            let pixel_grid = Stroke::new(0.5, Color32::from_rgba_unmultiplied(150, 150, 150, 120));
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
                            CONTROL_POINT_HOVER_RADIUS
                        } else {
                            CONTROL_POINT_RADIUS
                        };
                        if is_hovered {
                            painter.circle_filled(
                                self.doc_to_screen(rect, *point),
                                radius,
                                Color32::from_rgba_unmultiplied(255, 0, 100, 70),
                            );
                        }
                        painter.circle_stroke(
                            self.doc_to_screen(rect, *point),
                            radius,
                            Stroke::new(
                                if is_hovered { 2.5 } else { 1.5 },
                                Color32::from_rgb(255, 0, 100),
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
                    Stroke::new(2.0, Color32::from_rgb(255, 0, 100)),
                );
            }
        }
        if !self.pending.is_empty() {
            let guide_stroke = Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 0, 100, 150));
            for (index, point) in self.pending.iter().enumerate() {
                let screen_point = self.doc_to_screen(rect, *point);
                let radius = if index == 0 { 7.0 } else { 5.0 };
                painter.circle_filled(
                    screen_point,
                    radius,
                    Color32::from_rgba_unmultiplied(255, 0, 100, 25),
                );
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
                    // The expanded hover state is the source of truth for which
                    // control point is movable; do not use a separate drag hit test.
                    self.selected_point = hovered_control_point;
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
                    "polygon" => {
                        self.pending.push(p);
                    }
                    "polyline" | "path" | "rect" | "round_rect" | "ellipse" => {
                        self.pending.push(p);
                        if matches!(self.tool.as_str(), "rect" | "round_rect" | "ellipse")
                            && self.pending.len() == 2
                        {
                            self.commit_pending(false);
                        }
                    }
                    _ => {}
                }
            }
        }
        if response.secondary_clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.commit_pending(self.tool == "polygon");
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
        for layer in &self.doc.layers {
            if layer.visible {
                for object in &layer.objects {
                    self.draw_object_at(&painter, rect, object, 1.0, Vec2::ZERO);
                }
            }
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
        let (native_new, native_load, native_save) = self.native_menu.actions();
        let (shortcut_new, shortcut_load, shortcut_save, shortcut_copy_playdate_lua) =
            ui.input(|input| {
                let modifier = input.modifiers.ctrl || input.modifiers.command;
                (
                    modifier && input.key_pressed(egui::Key::N), // 新規作成.
                    modifier && input.key_pressed(egui::Key::O), // JSON読み込み.
                    modifier && input.key_pressed(egui::Key::S), // JSON保存.
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
        if shortcut_copy_playdate_lua {
            ui.ctx().copy_text(self.playdate_lua());
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
                });
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
                    "rect",
                    "round_rect",
                    "ellipse",
                    "path",
                ] {
                    if Self::tool_icon(ui, tool, self.tool == tool, self.fill).clicked() {
                        self.tool = tool.into();
                    }
                }
            });
            ui.checkbox(&mut self.fill, "Fill closed shape");
            ui.label(format!("Selected: {}", self.tool));
            ui.separator();
            ui.label("Style");
            ui.horizontal(|ui| {
                for color in ["black", "white", "clear"] {
                    if Self::color_icon(ui, color, self.color == color).clicked() {
                        self.color = color.into();
                    }
                }
            });
            let selected_dither = if self.tool == "select" {
                self.selected
                    .and_then(|(layer_index, object_index)| {
                        self.doc
                            .layers
                            .get(layer_index)
                            .and_then(|layer| layer.objects.get(object_index))
                    })
                    .map(|object| object.style.dither_pattern.clone())
            } else {
                None
            };
            let mut dither_pattern = selected_dither.unwrap_or_else(|| self.dither_pattern.clone());
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
            ui.add(egui::Slider::new(&mut self.width, 1..=8).text("Width"));
            if self.tool == "round_rect" {
                ui.add(egui::Slider::new(&mut self.radius, 0..=16).text("Corner radius"));
            }
            ui.checkbox(&mut self.pixel_preview, "Pixel preview");
            egui::ComboBox::from_id_salt("rounding")
                .selected_text(&self.rounding)
                .show_ui(ui, |ui| {
                    for mode in ["floor", "ceil", "nearest", "subpixel"] {
                        ui.selectable_value(&mut self.rounding, mode.into(), mode);
                    }
                });
            ui.separator();
            ui.label(format!("Zoom: {:.2}x", self.zoom));
            ui.horizontal(|ui| {
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
            });
            if ui.button("Finalize").clicked() {
                self.commit_pending(self.tool == "polygon");
            }
            if ui.button("Cancel").clicked() {
                self.pending.clear();
            }
            ui.horizontal(|ui| {
                if ui.button("Undo").clicked() {
                    self.undo_document();
                }
                if ui.button("Redo").clicked() {
                    self.redo_document();
                }
            });
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
            .default_size(430.0)
            .show(ui, |ui| {
                ui.heading("1-bit Preview");
                self.preview(ui);
                if ui.button("Copy Playdate Lua").clicked() {
                    ui.ctx().copy_text(self.playdate_lua());
                    self.status = "Copied Playdate Lua".into();
                }
                ui.separator();
                ui.heading("Vectors");
                let vector_names: Vec<String> = self.doc.layers[self.current_layer]
                    .objects
                    .iter()
                    .enumerate()
                    .map(|(index, object)| format!("{}: {}", index + 1, object.kind))
                    .collect();
                for (index, name) in vector_names.iter().enumerate() {
                    let is_selected = self.selected == Some((self.current_layer, index));
                    if ui.selectable_label(is_selected, name).clicked() {
                        self.selected = Some((self.current_layer, index));
                        self.selected_point = None;
                        self.tool = "select".into();
                    }
                }
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        self.delete_selected();
                    }
                    if ui.button("Duplicate").clicked() {
                        self.duplicate_selected();
                    }
                });
            });
        egui::containers::CentralPanel::default().show(ui, |ui| {
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
    }
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
            let mut app = DotStrokeApp::default();
            app.native_menu = native_menu;
            app.load_dither_icons(&_cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}
