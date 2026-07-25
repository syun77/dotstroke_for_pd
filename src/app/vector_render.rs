use super::*;

impl DotStrokeApp {
    pub(super) fn draw_object(&self, painter: &egui::Painter, rect: Rect, object: &VectorObject) {
        self.draw_object_at(painter, rect, object, self.zoom, self.pan);
    }

    pub(super) fn draw_object_at(
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

    pub(super) fn point_in_polygon_pos2(sample: Pos2, points: &[Pos2]) -> bool {
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

    pub(super) fn vector_dither_allows_sample(
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

    pub(super) fn draw_object_dithered_at(
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

    pub(super) fn draw_tool_preview(&self, painter: &egui::Painter, rect: Rect, cursor: Pos2) {
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
}
