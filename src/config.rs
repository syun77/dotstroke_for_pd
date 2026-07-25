use eframe::egui::Color32;

pub mod colors {
    use super::Color32;

    // 「clear」描画色（青系のガイドカラー）
    pub const CLEAR_COLOR_HEX: &str = "#3C96FF";

    // 描画中プレビューの線色
    pub const PREVIEW_STROKE_HEX: &str = "#FF006469";
    // 描画中プレビューの塗り色
    pub const PREVIEW_FILL_HEX: &str = "#FF006414";

    // ditherアイコン内チェッカーの濃いマス色
    pub const DITHER_SWATCH_CHECKER_DARK_HEX: &str = "#D7D7D7";
    // ditherアイコン内の斜線色
    pub const DITHER_SWATCH_DIAGONAL_HEX: &str = "#3C96FF";
    // 選択中のditherアイコン枠色（強調表示）
    pub const DITHER_SELECTED_BORDER_HEX: &str = "#FFB446";

    // グリッドの主線色
    pub const GRID_MAJOR_HEX: &str = "#CDCDCD";
    // グリッドの補助線色
    pub const GRID_MINOR_HEX: &str = "#E8E8E8";

    // 選択ハンドルの塗り色
    pub const SELECTION_FILL_HEX: &str = "#FF006446";
    // 選択ハンドルや選択線の線色
    pub const SELECTION_STROKE_HEX: &str = "#FF0064";

    // ピクセルプレビュー格子の線色
    pub const PIXEL_GRID_STROKE_HEX: &str = "#96969678";

    // ガイド点・ガイド線の線色
    pub const GUIDE_STROKE_HEX: &str = "#FF006496";
    // ガイド点の塗り色
    pub const GUIDE_FILL_HEX: &str = "#FF006419";

    // #RRGGBB または #RRGGBBAA の文字列を Color32 に変換する
    fn parse_hex_color(hex: &str) -> Color32 {
        let value = hex.strip_prefix('#').unwrap_or(hex);
        match value.len() {
            6 => {
                let r = u8::from_str_radix(&value[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&value[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&value[4..6], 16).unwrap_or(0);
                Color32::from_rgb(r, g, b)
            }
            8 => {
                let r = u8::from_str_radix(&value[0..2], 16).unwrap_or(0);
                let g = u8::from_str_radix(&value[2..4], 16).unwrap_or(0);
                let b = u8::from_str_radix(&value[4..6], 16).unwrap_or(0);
                let a = u8::from_str_radix(&value[6..8], 16).unwrap_or(255);
                Color32::from_rgba_unmultiplied(r, g, b, a)
            }
            _ => Color32::BLACK,
        }
    }

    pub fn clear_color() -> Color32 {
        parse_hex_color(CLEAR_COLOR_HEX)
    }

    pub fn preview_stroke() -> Color32 {
        parse_hex_color(PREVIEW_STROKE_HEX)
    }

    pub fn preview_fill() -> Color32 {
        parse_hex_color(PREVIEW_FILL_HEX)
    }

    pub fn dither_swatch_checker_dark() -> Color32 {
        parse_hex_color(DITHER_SWATCH_CHECKER_DARK_HEX)
    }

    pub fn dither_swatch_diagonal() -> Color32 {
        parse_hex_color(DITHER_SWATCH_DIAGONAL_HEX)
    }

    pub fn dither_selected_border() -> Color32 {
        parse_hex_color(DITHER_SELECTED_BORDER_HEX)
    }

    pub fn grid_major() -> Color32 {
        parse_hex_color(GRID_MAJOR_HEX)
    }

    pub fn grid_minor() -> Color32 {
        parse_hex_color(GRID_MINOR_HEX)
    }

    pub fn selection_fill() -> Color32 {
        parse_hex_color(SELECTION_FILL_HEX)
    }

    pub fn selection_stroke() -> Color32 {
        parse_hex_color(SELECTION_STROKE_HEX)
    }

    pub fn pixel_grid_stroke() -> Color32 {
        parse_hex_color(PIXEL_GRID_STROKE_HEX)
    }

    pub fn guide_stroke() -> Color32 {
        parse_hex_color(GUIDE_STROKE_HEX)
    }

    pub fn guide_fill() -> Color32 {
        parse_hex_color(GUIDE_FILL_HEX)
    }
}
