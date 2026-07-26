use super::*;

impl DotStrokeApp {
    pub(super) fn tool_icon(ui: &mut egui::Ui, tool: &str, selected: bool) -> egui::Response {
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

    pub(super) fn color_icon(ui: &mut egui::Ui, color: &str, selected: bool) -> egui::Response {
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

    pub(super) fn vector_row_icon(ui: &mut egui::Ui, object: &VectorObject, selected: bool) {
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

        if object.kind == "group" {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "▰",
                egui::FontId::proportional(14.0),
                ui.visuals().text_color(),
            );
            return;
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

    pub(super) fn vector_visibility_button(ui: &mut egui::Ui, visible: bool) -> egui::Response {
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

    pub(super) fn dither_icon(
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
}
