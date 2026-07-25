use super::*;

impl DotStrokeApp {
    pub(super) fn lua_number(value: f32) -> String {
        export::lua_number(value)
    }

    pub(super) fn lua_cap_style(cap: &str) -> &'static str {
        export::lua_cap_style(cap)
    }

    pub(super) fn lua_style_fields(object: &VectorObject) -> Vec<String> {
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

    pub(super) fn append_lua_object_with_animation_function(
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

    pub(super) fn append_lua_object_simple(
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

    pub(super) fn last_visible_lua_color(object: &VectorObject) -> Option<&str> {
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
    pub(super) fn collect_animation_kinds(object: &VectorObject, kinds: &mut HashSet<String>) {
        if object.visible && !object.points.is_empty() {
            kinds.insert(object.kind.clone());
        }
        for child in &object.children {
            Self::collect_animation_kinds(child, kinds);
        }
    }

    #[allow(dead_code)]
    pub(super) fn animation_primitive_lua(&self) -> String {
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

    pub(super) fn append_animation_object_inline(
        &self,
        output: &mut String,
        object: &VectorObject,
    ) {
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

    pub(super) fn animation_single_function_lua(&self) -> String {
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

    pub(super) fn playdate_lua(&self, animation: bool) -> String {
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
}
