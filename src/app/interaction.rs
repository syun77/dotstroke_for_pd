use super::*;

impl DotStrokeApp {
    pub(super) fn has_multiple_selected_objects(&self) -> bool {
        self.selected_objects.len() > 1
    }

    pub(super) fn select_single_object(&mut self, object_index: usize) {
        self.selected = Some((self.current_layer, object_index));
        self.selected_objects.clear();
        self.selected_objects.push(object_index);
        self.selected_point = None;
    }

    pub(super) fn toggle_object_selection(&mut self, object_index: usize) {
        if self.selected_objects.is_empty() {
            if let Some((layer_index, selected_index)) = self.selected {
                if layer_index == self.current_layer {
                    self.selected_objects.push(selected_index);
                }
            }
        }
        if let Some(position) = self
            .selected_objects
            .iter()
            .position(|index| *index == object_index)
        {
            self.selected_objects.remove(position);
        } else {
            self.selected_objects.push(object_index);
        }
        self.selected = self
            .selected_objects
            .last()
            .copied()
            .map(|index| (self.current_layer, index));
        self.selected_point = None;
    }

    pub(super) fn select_object_range(&mut self, object_index: usize) {
        let anchor = self
            .selected_objects
            .first()
            .copied()
            .or_else(|| self.selected.map(|(_, index)| index));
        let Some(anchor) = anchor else {
            self.select_single_object(object_index);
            return;
        };

        let start = anchor.min(object_index);
        let end = anchor.max(object_index);
        self.selected_objects = (start..=end).collect();
        self.selected = Some((self.current_layer, object_index));
        self.selected_point = None;
    }

    fn snap_value(&self, value: f32) -> f32 {
        match self.rounding.as_str() {
            "floor" => value.floor(),
            "ceil" => value.ceil(),
            _ => value.round(),
        }
    }

    pub(super) fn snap(&self, p: Pos2) -> [f32; 2] {
        [self.snap_value(p.x), self.snap_value(p.y)]
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
        if self.has_multiple_selected_objects() {
            return None;
        }
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
        let layer_index = self.current_layer;
        let indices = if self.selected_objects.is_empty() {
            self.selected
                .filter(|(selected_layer, _)| *selected_layer == layer_index)
                .map(|(_, index)| vec![index])
                .unwrap_or_default()
        } else {
            self.selected_objects.clone()
        };
        if let Some(layer) = self.doc.layers.get_mut(layer_index) {
            for object_index in indices {
                if let Some(object) = layer.objects.get_mut(object_index) {
                    editor::move_object(object, delta);
                }
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

    pub(super) fn snap_selected_geometry_to_grid(&mut self) {
        let layer_index = self.current_layer;
        let indices = if self.selected_objects.is_empty() {
            self.selected
                .filter(|(selected_layer, _)| *selected_layer == layer_index)
                .map(|(_, index)| vec![index])
                .unwrap_or_default()
        } else {
            self.selected_objects.clone()
        };
        if indices.is_empty() {
            return;
        }
        let single_selection = indices.len() == 1;
        let selected_point = if single_selection {
            self.selected_point
        } else {
            None
        };
        let rounding = self.rounding.as_str();
        let snap_value = |value: f32| match rounding {
            "floor" => value.floor(),
            "ceil" => value.ceil(),
            _ => value.round(),
        };
        let Some(layer) = self.doc.layers.get_mut(layer_index) else {
            return;
        };

        for object_index in indices {
            let Some(object) = layer.objects.get_mut(object_index) else {
                continue;
            };
            if let Some(point_index) = selected_point {
                if let Some(point) = object.points.get_mut(point_index) {
                    point[0] = snap_value(point[0]);
                    point[1] = snap_value(point[1]);
                }
            } else {
                for point in &mut object.points {
                    point[0] = snap_value(point[0]);
                    point[1] = snap_value(point[1]);
                }
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
                self.selected_objects.clear();
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
                self.selected_objects = vec![new_index];
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
                for index in &mut self.selected_objects {
                    if *index == from {
                        *index = to;
                    } else if from < to && *index > from && *index <= to {
                        *index -= 1;
                    } else if to < from && *index >= to && *index < from {
                        *index += 1;
                    }
                }
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
