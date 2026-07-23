use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use serde::{Deserialize, Serialize};
use std::fs;

const DEFAULT_WIDTH: i32 = 400;
const DEFAULT_HEIGHT: i32 = 240;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct Style {
    color: String,
    blend: String,
    width: i32,
    cap: String,
    fill: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            color: "black".into(),
            blend: "normal".into(),
            width: 1,
            cap: "butt".into(),
            fill: false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct VectorObject {
    #[serde(rename = "type")]
    kind: String,
    points: Vec<[f32; 2]>,
    closed: bool,
    style: Style,
    transform: serde_json::Value,
    children: Vec<VectorObject>,
    visible: bool,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct Layer {
    id: String,
    visible: bool,
    objects: Vec<VectorObject>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct Target {
    sdk: String,
    width: i32,
    height: i32,
    coordinate_system: String,
    pixel_snap: String,
    rounding: String,
    clip: bool,
}

impl Default for Target {
    fn default() -> Self {
        Self {
            sdk: "3.1.1".into(),
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            coordinate_system: "top-left".into(),
            pixel_snap: "integer".into(),
            rounding: "nearest".into(),
            clip: true,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct Document {
    format: String,
    version: i32,
    target: Target,
    canvas: serde_json::Value,
    optimize: serde_json::Value,
    layers: Vec<Layer>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            format: "pdvector".into(),
            version: 1,
            target: Target::default(),
            canvas: serde_json::json!({"background":"white", "ditherAnchor":"screen"}),
            optimize: serde_json::json!({"mergeCollinearLines":true, "removeDuplicatePoints":true, "simplifyTolerance":0}),
            layers: vec![Layer {
                id: "layer1".into(),
                visible: true,
                objects: vec![],
            }],
        }
    }
}

struct DotStrokeApp {
    doc: Document,
    tool: String,
    color: String,
    width: i32,
    fill: bool,
    rounding: String,
    zoom: f32,
    pan: Vec2,
    viewport_size: Vec2,
    pending: Vec<[f32; 2]>,
    selected: Option<(usize, usize)>,
    current_layer: usize,
    status: String,
    undo: Vec<Document>,
}

impl Default for DotStrokeApp {
    fn default() -> Self {
        Self {
            doc: Document::default(),
            tool: "line".into(),
            color: "black".into(),
            width: 1,
            fill: false,
            rounding: "nearest".into(),
            zoom: 2.0,
            pan: Vec2::ZERO,
            viewport_size: Vec2::new(800.0, 600.0),
            pending: vec![],
            selected: None,
            current_layer: 0,
            status: "Ready".into(),
            undo: vec![],
        }
    }
}

impl DotStrokeApp {
    fn save_history(&mut self) {
        self.undo.push(self.doc.clone());
        if self.undo.len() > 100 {
            self.undo.remove(0);
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
        Pos2::new(
            (p.x - rect.left() - self.pan.x) / self.zoom,
            (p.y - rect.top() - self.pan.y) / self.zoom,
        )
    }

    fn doc_to_screen(&self, rect: Rect, p: [f32; 2]) -> Pos2 {
        Pos2::new(
            rect.left() + self.pan.x + p[0] * self.zoom,
            rect.top() + self.pan.y + p[1] * self.zoom,
        )
    }

    fn max_zoom(&self) -> f32 {
        (self.viewport_size.x.min(self.viewport_size.y) / 8.0).max(0.25)
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

    fn move_selected(&mut self, delta: Vec2) {
        if let Some((layer_index, object_index)) = self.selected {
            if let Some(object) = self
                .doc
                .layers
                .get_mut(layer_index)
                .and_then(|l| l.objects.get_mut(object_index))
            {
                for point in &mut object.points {
                    point[0] += delta.x;
                    point[1] += delta.y;
                }
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
                ..Style::default()
            },
            visible: true,
            ..Default::default()
        });
        self.status = "Vector added".into();
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
            "ellipse" if pts.len() >= 2 => {
                let r = Rect::from_two_pos(pts[0], pts[1]);
                painter.circle_stroke(r.center(), r.width().min(r.height()) / 2.0, stroke);
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

    fn draw_canvas(&mut self, ui: &mut egui::Ui) {
        let size = ui.available_size();
        self.viewport_size = size;
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
        for layer in &self.doc.layers {
            if layer.visible {
                for (index, object) in layer.objects.iter().enumerate() {
                    self.draw_object(&painter, rect, object);
                    if self.selected == Some((self.current_layer, index))
                        && !object.points.is_empty()
                    {
                        for point in &object.points {
                            painter.circle_stroke(
                                self.doc_to_screen(rect, *point),
                                4.0,
                                Stroke::new(1.5, Color32::from_rgb(255, 0, 100)),
                            );
                        }
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
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.0 {
                self.zoom = (self.zoom * (1.0 + scroll.signum() * 0.1)).clamp(0.25, max_zoom);
            }
        }
        let space_down = ui.input(|i| i.key_down(egui::Key::Space));
        if (space_down && response.dragged_by(egui::PointerButton::Primary))
            || response.dragged_by(egui::PointerButton::Middle)
        {
            self.pan += ui.input(|i| i.pointer.delta());
        }
        if self.tool == "select" && !space_down && response.dragged_by(egui::PointerButton::Primary)
        {
            if self.selected.is_some() {
                if response.drag_started() {
                    self.save_history();
                }
                let delta = ui.input(|i| i.pointer.delta()) / self.zoom;
                self.move_selected(delta);
            }
        } else if self.tool == "select" && !space_down && response.clicked() {
            self.selected = response
                .interact_pointer_pos()
                .and_then(|pos| self.hit_test(rect, pos))
                .map(|index| (self.current_layer, index));
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
                    "polyline" | "path" | "rect" | "ellipse" => {
                        self.pending.push(p);
                        if matches!(self.tool.as_str(), "rect" | "ellipse")
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
        egui::Panel::left("tools").resizable(false).show(ui, |ui| {
            ui.heading("DotStroke");
            ui.label("Tool");
            egui::ComboBox::from_id_salt("tool")
                .selected_text(&self.tool)
                .show_ui(ui, |ui| {
                    for tool in [
                        "select", "pixel", "line", "polyline", "polygon", "rect", "ellipse",
                        "bezier", "path",
                    ] {
                        ui.selectable_value(&mut self.tool, tool.into(), tool);
                    }
                });
            ui.separator();
            ui.label("Resolution");
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} x {}",
                    self.doc.target.width, self.doc.target.height
                ));
                if ui.button("32x32").clicked() {
                    self.doc.target.width = 32;
                    self.doc.target.height = 32;
                }
                if ui.button("400x240").clicked() {
                    self.doc.target.width = 400;
                    self.doc.target.height = 240;
                }
            });
            ui.separator();
            ui.label("Style");
            egui::ComboBox::from_id_salt("color")
                .selected_text(&self.color)
                .show_ui(ui, |ui| {
                    for c in ["black", "white", "clear"] {
                        ui.selectable_value(&mut self.color, c.into(), c);
                    }
                });
            ui.add(egui::Slider::new(&mut self.width, 1..=8).text("Width"));
            ui.checkbox(&mut self.fill, "Fill closed shape");
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
            ui.separator();
            ui.label("Vectors");
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
            ui.separator();
            if ui.button("New").clicked() {
                self.save_history();
                self.doc = Document::default();
                self.pending.clear();
            }
            if ui.button("Load JSON").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("JSON", &["json"])
                    .pick_file()
                {
                    match fs::read_to_string(&path)
                        .ok()
                        .and_then(|s| serde_json::from_str::<Document>(&s).ok())
                    {
                        Some(doc) => {
                            self.save_history();
                            self.doc = doc;
                            self.status = format!("Loaded {}", path.display());
                        }
                        None => self.status = "Failed to load JSON".into(),
                    }
                }
            }
            if ui.button("Save JSON").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name("document.json")
                    .save_file()
                {
                    match serde_json::to_string_pretty(&self.doc)
                        .ok()
                        .and_then(|s| fs::write(&path, s).ok())
                    {
                        Some(_) => self.status = format!("Saved {}", path.display()),
                        None => self.status = "Failed to save JSON".into(),
                    }
                }
            }
            ui.separator();
            ui.label(&self.status);
            ui.label("Space + left-drag: pan");
            ui.label("Middle-drag: pan");
            ui.label("Wheel: zoom");
            ui.label("Right-click/Enter: finalize");
        });
        egui::Panel::right("preview")
            .resizable(false)
            .default_size(430.0)
            .show(ui, |ui| {
                ui.heading("1-bit Preview");
                self.preview(ui);
            });
        egui::containers::CentralPanel::default().show(ui, |ui| {
            self.draw_canvas(ui);
        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("DotStroke for Playdate (egui)")
            .with_inner_size([1500.0, 760.0]),
        ..Default::default()
    };
    eframe::run_native(
        "DotStroke",
        options,
        Box::new(|_cc| Ok(Box::new(DotStrokeApp::default()))),
    )
}
