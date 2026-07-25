/// Playdate Lua formatting helpers. Kept independent from egui and the application state.
pub fn lua_number(value: f32) -> String {
    if value == 0.0 {
        "0".into()
    } else {
        value.to_string()
    }
}
pub fn lua_color(color: &str) -> &'static str {
    match color {
        "white" => "gfx.kColorWhite",
        "clear" => "gfx.kColorClear",
        _ => "gfx.kColorBlack",
    }
}

pub fn lua_color_with_blend(color: &str, blend: &str) -> &'static str {
    if blend == "xor" {
        "gfx.kColorXOR"
    } else {
        lua_color(color)
    }
}

pub fn lua_cap_style(cap: &str) -> &'static str {
    match cap {
        "round" => "gfx.kLineCapStyleRound",
        "square" => "gfx.kLineCapStyleSquare",
        _ => "gfx.kLineCapStyleButt",
    }
}
pub fn lua_dither_pattern(pattern: &str) -> Option<&'static str> {
    match pattern {
        "diagonal_line" => Some("gfx.image.kDitherTypeDiagonalLine"),
        "vertical_line" => Some("gfx.image.kDitherTypeVerticalLine"),
        "horizontal_line" => Some("gfx.image.kDitherTypeHorizontalLine"),
        "screen" => Some("gfx.image.kDitherTypeScreen"),
        "bayer_2x2" => Some("gfx.image.kDitherTypeBayer2x2"),
        "bayer_4x4" => Some("gfx.image.kDitherTypeBayer4x4"),
        "bayer_8x8" => Some("gfx.image.kDitherTypeBayer8x8"),
        "floyd_steinberg" => Some("gfx.image.kDitherTypeFloydSteinberg"),
        "burkes" => Some("gfx.image.kDitherTypeBurkes"),
        "atkinson" => Some("gfx.image.kDitherTypeAtkinson"),
        _ => None,
    }
}
