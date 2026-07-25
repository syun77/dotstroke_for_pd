use eframe::egui::Color32;
// =======================================================
// パラメータ設定ファイル.
// =======================================================

// ■操作関連.
pub mod interaction {
    // 制御点の通常表示半径
    pub const CONTROL_POINT_RADIUS: f32 = 4.0;
    // 制御点ホバー時の表示半径
    pub const CONTROL_POINT_HOVER_RADIUS: f32 = 9.0;
    // 制御点のホバー/ドラッグ判定半径の上限（大きめに設定）
    pub const CONTROL_POINT_HIT_RADIUS: f32 = 40.0;
}

// ■UIレイアウト・フォント関連.
pub mod ui {
    // Previewパネルの幅
    pub const PREVIEW_PANEL_WIDTH: f32 = 520.0;
    // Vector一覧の最低表示高さ。項目が少なくてもドラッグしやすい領域を確保する
    pub const VECTOR_LIST_MIN_HEIGHT: f32 = 300.0;
    // Vector一覧1行の高さ
    pub const VECTOR_ROW_HEIGHT: f32 = 30.0;
    // ドラッグハンドルの幅
    pub const VECTOR_DRAG_HANDLE_WIDTH: f32 = 32.0;
    // 表示/非表示ボタンの幅
    pub const VECTOR_VISIBILITY_WIDTH: f32 = 28.0;
    // 行末の上下ボタン領域。ここを除いたアイコン・名前部分がドラッグ対象
    pub const VECTOR_ROW_ACTION_WIDTH: f32 = 70.0;
    // 最近使ったファイルのパスを表示するメニュー幅
    pub const RECENT_FILES_MENU_WIDTH: f32 = 600.0;

    // UIの標準フォントサイズ
    pub const FONT_SIZE_BODY: f32 = 16.0;
    pub const FONT_SIZE_BUTTON: f32 = 16.0;
    pub const FONT_SIZE_HEADING: f32 = 21.0;
    pub const FONT_SIZE_SMALL: f32 = 14.0;
    pub const FONT_SIZE_MONOSPACE: f32 = 14.0;
}

// ■色関連.
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
    // 透明部分を示すチェッカーボードの明るいマス色
    pub const TRANSPARENCY_CHECKER_LIGHT_HEX: &str = "#8e8e8e";
    // 透明部分を示すチェッカーボードの薄い灰色マス色
    pub const TRANSPARENCY_CHECKER_DARK_HEX: &str = "#dcdcdc";
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

    pub fn transparency_checker_light() -> Color32 {
        parse_hex_color(TRANSPARENCY_CHECKER_LIGHT_HEX)
    }

    pub fn transparency_checker_dark() -> Color32 {
        parse_hex_color(TRANSPARENCY_CHECKER_DARK_HEX)
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
