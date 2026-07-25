use super::*;

impl DotStrokeApp {
    pub(super) fn apply_raster_blend_color(
        current: Color32,
        color: Color32,
        blend: &str,
    ) -> Color32 {
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

    pub(super) fn put_raster_pixel(
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

    pub(super) fn bayer_4x4_threshold(x: usize, y: usize) -> u8 {
        const MATRIX: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
        MATRIX[y % 4][x % 4]
    }

    pub(super) fn bayer_8x8_threshold(x: usize, y: usize) -> u8 {
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

    pub(super) fn dither_allows_pixel(pattern: &str, x: usize, y: usize) -> bool {
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

    pub(super) fn draw_raster_line(
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

    pub(super) fn draw_raster_ellipse(
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

    pub(super) fn rasterize_object(
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

    pub(super) fn pixel_preview_with_background(
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
}
