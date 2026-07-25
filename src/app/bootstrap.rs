use super::*;

impl DotStrokeApp {
    fn load_dither_icons(&mut self, ctx: &egui::Context) {
        let icon_dirs = [
            PathBuf::from(DITHER_ICON_DIR),
            Path::new(env!("CARGO_MANIFEST_DIR")).join(DITHER_ICON_DIR),
        ];
        for pattern in DITHER_PATTERNS {
            let Some(path) = icon_dirs
                .iter()
                .map(|dir| dir.join(format!("{pattern}.png")))
                .find(|path| path.is_file())
            else {
                continue;
            };
            let Ok(bytes) = fs::read(&path) else {
                continue;
            };
            let Ok(decoded) = image::load_from_memory(&bytes) else {
                continue;
            };
            let rgba = decoded.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            let texture = ctx.load_texture(
                format!("dither-icon-{pattern}"),
                color_image,
                egui::TextureOptions::NEAREST,
            );
            self.dither_icons.insert(pattern.into(), texture);
        }
    }
}

fn configure_fonts(ctx: &egui::Context) {
    // egui's built-in font is intentionally small and does not contain all
    // arrows, Japanese characters, or other UI symbols. Add a system CJK font
    // as a fallback so these glyphs render instead of becoming tofu boxes.
    const SYSTEM_FONT_CANDIDATES: [&str; 8] = [
        "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
        "/System/Library/Fonts/AppleSDGothicNeo.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJKjp-Regular.otf",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "C:\\Windows\\Fonts\\meiryo.ttc",
        "C:\\Windows\\Fonts\\msgothic.ttc",
        "C:\\Windows\\Fonts\\YuGothR.ttc",
    ];

    let system_font = SYSTEM_FONT_CANDIDATES
        .iter()
        .find_map(|path| fs::read(path).ok().map(|bytes| (path.to_string(), bytes)));

    let mut fonts = egui::FontDefinitions::default();
    if let Some((font_name, font_bytes)) = system_font {
        fonts.font_data.insert(
            font_name.clone(),
            egui::FontData::from_owned(font_bytes).into(),
        );
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            if let Some(family_fonts) = fonts.families.get_mut(&family) {
                family_fonts.push(font_name.clone());
            }
        }
    }
    ctx.set_fonts(fonts);

    ctx.all_styles_mut(|style| {
        style.text_styles.insert(
            egui::TextStyle::Body,
            egui::FontId::proportional(config::ui::FONT_SIZE_BODY),
        );
        style.text_styles.insert(
            egui::TextStyle::Button,
            egui::FontId::proportional(config::ui::FONT_SIZE_BUTTON),
        );
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::proportional(config::ui::FONT_SIZE_HEADING),
        );
        style.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::proportional(config::ui::FONT_SIZE_SMALL),
        );
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::monospace(config::ui::FONT_SIZE_MONOSPACE),
        );
    });
}

pub fn run() -> eframe::Result {
    let native_menu = ui::NativeMenu::new();
    native_menu.init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("DotStroke for Playdate (egui)")
            .with_inner_size([1500.0, 760.0]),
        event_loop_builder: Some(Box::new(|builder| {
            #[cfg(target_os = "macos")]
            {
                use winit::platform::macos::EventLoopBuilderExtMacOS;
                builder.with_default_menu(false);
            }
        })),
        ..Default::default()
    };
    eframe::run_native(
        "DotStroke",
        options,
        Box::new(move |_cc| {
            configure_fonts(&_cc.egui_ctx);
            let mut app = DotStrokeApp::default();
            app.native_menu = native_menu;
            app.load_dither_icons(&_cc.egui_ctx);
            app.restore_last_document();
            Ok(Box::new(app))
        }),
    )
}
