use super::*;

impl DotStrokeApp {
    pub(super) fn draw_transparency_checkerboard(
        painter: &egui::Painter,
        rect: Rect,
        cell_size: f32,
    ) {
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

    pub(super) fn draw_canvas(&mut self, ui: &mut egui::Ui) {
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

    pub(super) fn preview(&self, ui: &mut egui::Ui) {
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
