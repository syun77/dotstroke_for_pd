use super::*;

impl DotStrokeApp {
    pub(super) fn snap(&self, p: Pos2) -> [f32; 2] {
        match self.rounding.as_str() {
            "floor" => [p.x.floor(), p.y.floor()],
            "ceil" => [p.x.ceil(), p.y.ceil()],
            _ => [p.x.round(), p.y.round()],
        }
    }

    pub(super) fn screen_to_doc(&self, rect: Rect, p: Pos2) -> Pos2 {
        render::ViewTransform {
            zoom: self.zoom,
            pan: self.pan,
        }
        .screen_to_document(rect, p)
    }

    pub(super) fn doc_to_screen(&self, rect: Rect, p: [f32; 2]) -> Pos2 {
        render::ViewTransform {
            zoom: self.zoom,
            pan: self.pan,
        }
        .document_to_screen(rect, p)
    }

    pub(super) fn max_zoom(&self) -> f32 {
        render::ViewTransform::max_zoom(self.viewport_size)
    }

    pub(super) fn fit_canvas_to_viewport(&mut self) {
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

    pub(super) fn hit_test(&self, rect: Rect, pos: Pos2) -> Option<usize> {
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

    pub(super) fn hit_test_control_point(&self, rect: Rect, pos: Pos2) -> Option<usize> {
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

    pub(super) fn move_selected(&mut self, delta: Vec2) {
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

    pub(super) fn move_selected_point(&mut self, point_index: usize, delta: Vec2) {
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

    pub(super) fn delete_selected(&mut self) {
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

    pub(super) fn duplicate_selected(&mut self) {
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

    pub(super) fn reorder_vector(&mut self, layer_index: usize, from: usize, to: usize) {
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

    pub(super) fn paint_vector_drag_preview(&self, ctx: &egui::Context) {
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

    pub(super) fn commit_pending(&mut self, closed: bool) {
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
}
