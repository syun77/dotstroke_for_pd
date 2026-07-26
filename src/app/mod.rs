use crate::editor::History;
use crate::model::{Document, Style, VectorObject, DEFAULT_HEIGHT, DEFAULT_WIDTH};
use crate::{config, editor, export, io, render, ui};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

mod bootstrap;
mod canvas;
mod document;
mod interaction;
mod lua_export;
mod raster;
mod reference;
mod state;
mod vector_render;
mod widgets;

pub use bootstrap::run;
use state::DotStrokeApp;

const DITHER_PATTERNS: [&str; 11] = [
    "none",
    "diagonal_line",
    "vertical_line",
    "horizontal_line",
    "screen",
    "bayer_2x2",
    "bayer_4x4",
    "bayer_8x8",
    "floyd_steinberg",
    "burkes",
    "atkinson",
];
const DITHER_ICON_DIR: &str = "assets/dither_icons";

impl DotStrokeApp {
    fn current_tool_kind(&self) -> &str {
        match self.tool.as_str() {
            "fill_rect" => "rect",
            "fill_round_rect" => "round_rect",
            "fill_circle" => "ellipse",
            "fill_polygon" => "polygon",
            _ => self.tool.as_str(),
        }
    }

    fn current_tool_fill(&self) -> bool {
        matches!(
            self.tool.as_str(),
            "fill_rect" | "fill_round_rect" | "fill_circle" | "fill_polygon"
        )
    }

    fn current_tool_label(&self) -> &str {
        match self.tool.as_str() {
            "fill_rect" => "fill rect",
            "fill_round_rect" => "fill round rect",
            "fill_circle" => "fill circle",
            "fill_polygon" => "fill polygon",
            _ => self.tool.as_str(),
        }
    }
}

impl eframe::App for DotStrokeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        #[cfg(target_os = "macos")]
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
        let close_requested = ui.input(|input| input.viewport().close_requested());
        if close_requested && !self.allow_close && self.document_is_dirty() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_dialog = true;
        }
        let (undo_pressed, redo_pressed) = ui.input(|input| {
            let modifier = input.modifiers.ctrl || input.modifiers.command;
            (
                modifier && input.key_pressed(egui::Key::Z),
                modifier && input.key_pressed(egui::Key::Y),
            )
        });
        if undo_pressed {
            self.undo_document();
        }
        if redo_pressed {
            self.redo_document();
        }
        let paste_reference = ui.input(|input| {
            let modifier = input.modifiers.ctrl || input.modifiers.command;
            modifier && input.key_pressed(egui::Key::V)
        });
        if paste_reference {
            self.add_reference_clipboard(ui.ctx());
        }
        let dropped_files: Vec<(String, Option<Vec<u8>>)> = ui.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| {
                    let name = file
                        .path
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "Dropped image".into());
                    (name, file.bytes.as_ref().map(|bytes| bytes.to_vec()))
                })
                .collect()
        });
        for (name, bytes) in dropped_files {
            if let Some(bytes) = bytes {
                self.add_reference_image(ui.ctx(), name, &bytes);
            } else if let Some(path) = PathBuf::from(&name).canonicalize().ok() {
                if let Ok(bytes) = fs::read(&path) {
                    self.add_reference_image(ui.ctx(), path.display().to_string(), &bytes);
                }
            }
        }
        let (native_new, native_load, native_save, native_save_as, native_export_png) =
            self.native_menu.actions();
        let (
            shortcut_new,
            shortcut_load,
            shortcut_save,
            shortcut_save_as,
            shortcut_export_png,
            shortcut_copy_playdate_lua,
        ) = ui.input(|input| {
            let modifier = input.modifiers.ctrl || input.modifiers.command;
            (
                modifier && input.key_pressed(egui::Key::N), // 新規作成.
                modifier && input.key_pressed(egui::Key::O), // JSON読み込み.
                modifier && !input.modifiers.shift && input.key_pressed(egui::Key::S), // JSON保存.
                modifier && input.modifiers.shift && input.key_pressed(egui::Key::S), // 名前を付けて保存.
                modifier && input.modifiers.shift && input.key_pressed(egui::Key::E), // PNGエクスポート.
                modifier && input.key_pressed(egui::Key::P), // Copy Playdate Lua.
            )
        });
        if native_new || shortcut_new {
            self.begin_new_document();
        }
        if native_load || shortcut_load {
            self.load_json_document();
        }
        if native_save || shortcut_save {
            self.save_json_document();
        }
        if native_save_as || shortcut_save_as {
            self.save_json_document_as();
        }
        if native_export_png || shortcut_export_png {
            self.export_png();
        }
        if shortcut_copy_playdate_lua {
            ui.ctx()
                .copy_text(self.playdate_lua(false, self.doc.offset));
            self.status = "Copied Playdate Lua".into();
        }
        egui::Panel::top("menu").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New    Cmd+N").clicked() {
                        self.begin_new_document();
                        ui.close();
                    }
                    if ui.button("Load JSON    Cmd+O").clicked() {
                        self.load_json_document();
                        ui.close();
                    }
                    ui.menu_button("Open Recent", |ui| {
                        ui.set_min_width(config::ui::RECENT_FILES_MENU_WIDTH);
                        let recent_files = self.recent_files.clone();
                        if recent_files.is_empty() {
                            ui.label("No recent files");
                        } else {
                            for path in recent_files {
                                let label = path.display().to_string();
                                if ui.button(label).clicked() {
                                    self.load_json_document_from_path(&path);
                                    ui.close();
                                }
                            }
                            ui.separator();
                            if ui.button("Clear Recent Files").clicked() {
                                self.clear_recent_files();
                                ui.close();
                            }
                        }
                    });
                    if ui.button("Save JSON    Cmd+S").clicked() {
                        self.save_json_document();
                        ui.close();
                    }
                    if ui.button("Save JSON As    Cmd+Shift+S").clicked() {
                        self.save_json_document_as();
                        ui.close();
                    }
                    if ui.button("Export PNG    Cmd+Shift+E").clicked() {
                        self.export_png();
                        ui.close();
                    }
                });
                if ui.button("Reference Preview").clicked() {
                    self.reference_window = true;
                    self.reference_focus_requested = true;
                }
                ui.menu_button("Resolution", |ui| {
                    ui.label(format!(
                        "Current: {} x {}",
                        self.doc.target.width, self.doc.target.height
                    ));
                    ui.separator();
                    if ui.button("32 x 32").clicked() {
                        self.save_history();
                        self.doc.target.width = 32;
                        self.doc.target.height = 32;
                        ui.close();
                    }
                    if ui.button("400 x 240").clicked() {
                        self.save_history();
                        self.doc.target.width = 400;
                        self.doc.target.height = 240;
                        self.last_fitted_target = None;
                        ui.close();
                    }
                    if ui.button("Change Resolution…").clicked() {
                        self.begin_change_resolution();
                        ui.close();
                    }
                });
            });
        });
        egui::Panel::left("tools").resizable(false).show(ui, |ui| {
            ui.heading("DotStroke");
            ui.label("Tool");
            ui.horizontal_wrapped(|ui| {
                for tool in [
                    "select",
                    "pixel",
                    "line",
                    "polyline",
                    "polygon",
                    "fill_polygon",
                    "rect",
                    "fill_rect",
                    "round_rect",
                    "fill_round_rect",
                    "ellipse",
                    "fill_circle",
                    "path",
                ] {
                    if Self::tool_icon(ui, tool, self.tool == tool).clicked() {
                        self.tool = tool.into();
                    }
                }
            });
            ui.label(format!("Selected: {}", self.current_tool_label()));
            ui.separator();
            ui.label("Style");
            let selected_style = if self.tool == "select" {
                self.selected
                    .and_then(|(layer_index, object_index)| {
                        self.doc
                            .layers
                            .get(layer_index)
                            .and_then(|layer| layer.objects.get(object_index))
                    })
                    .map(|object| object.style.clone())
            } else {
                None
            };
            let selected_kind = if self.tool == "select" {
                self.selected
                    .and_then(|(layer_index, object_index)| {
                        self.doc
                            .layers
                            .get(layer_index)
                            .and_then(|layer| layer.objects.get(object_index))
                    })
                    .map(|object| object.kind.clone())
            } else {
                None
            };
            let mut color = selected_style
                .as_ref()
                .map(|style| style.color.clone())
                .unwrap_or_else(|| self.color.clone());
            let original_color = color.clone();
            ui.horizontal(|ui| {
                for color_name in ["black", "white", "clear"] {
                    if Self::color_icon(ui, color_name, color == color_name).clicked() {
                        color = color_name.into();
                    }
                }
            });
            let color_changed = color != original_color;
            if color_changed {
                self.color = color.clone();
                if let Some((layer_index, object_index)) = self.selected {
                    if self.tool == "select" {
                        self.save_history();
                        if let Some(object) = self
                            .doc
                            .layers
                            .get_mut(layer_index)
                            .and_then(|layer| layer.objects.get_mut(object_index))
                        {
                            object.style.color = color;
                            self.status = "Color changed".into();
                        }
                    }
                }
            }

            let mut blend = selected_style
                .as_ref()
                .map(|style| style.blend.clone())
                .unwrap_or_else(|| self.blend.clone());
            let original_blend = blend.clone();
            ui.horizontal(|ui| {
                ui.label("Blend");
                ui.selectable_value(&mut blend, "normal".into(), "Normal");
                ui.selectable_value(&mut blend, "xor".into(), "XOR");
            });
            let blend_changed = blend != original_blend;
            if blend_changed {
                self.blend = blend.clone();
                if self.tool == "select" {
                    if let Some((layer_index, object_index)) = self.selected {
                        self.save_history();
                        if let Some(object) = self
                            .doc
                            .layers
                            .get_mut(layer_index)
                            .and_then(|layer| layer.objects.get_mut(object_index))
                        {
                            object.style.blend = blend;
                            self.status = "Blend mode changed".into();
                        }
                    }
                }
            }

            let mut dither_pattern = selected_style
                .as_ref()
                .map(|style| style.dither_pattern.clone())
                .unwrap_or_else(|| self.dither_pattern.clone());
            let original_dither_pattern = dither_pattern.clone();
            ui.label("Dither pattern");
            ui.horizontal_wrapped(|ui| {
                for pattern in DITHER_PATTERNS {
                    if Self::dither_icon(
                        ui,
                        pattern,
                        dither_pattern == pattern,
                        self.dither_icons.get(pattern),
                    )
                    .clicked()
                    {
                        dither_pattern = pattern.into();
                    }
                }
            });
            let dither_changed = dither_pattern != original_dither_pattern;
            if dither_changed {
                self.dither_pattern = dither_pattern.clone();
                if self.tool == "select" {
                    if let Some((layer_index, object_index)) = self.selected {
                        self.save_history();
                        if let Some(object) = self
                            .doc
                            .layers
                            .get_mut(layer_index)
                            .and_then(|layer| layer.objects.get_mut(object_index))
                        {
                            object.style.dither_pattern = dither_pattern;
                            self.status = "Dither pattern changed".into();
                        }
                    }
                }
            }
            let mut width = selected_style
                .as_ref()
                .map_or(self.width, |style| style.width);
            let width_response = ui.add(egui::Slider::new(&mut width, 1..=8).text("Width"));
            if width_response.changed() {
                self.width = width;
                if self.tool == "select" {
                    if let Some((layer_index, object_index)) = self.selected {
                        self.save_history();
                        if let Some(object) = self
                            .doc
                            .layers
                            .get_mut(layer_index)
                            .and_then(|layer| layer.objects.get_mut(object_index))
                        {
                            object.style.width = width;
                            self.status = "Width changed".into();
                        }
                    }
                }
            } else if self.tool != "select" {
                self.width = width;
            }

            if matches!(self.tool.as_str(), "round_rect" | "fill_round_rect")
                || selected_style.as_ref().is_some()
                    && matches!(selected_kind.as_deref(), Some("round_rect"))
            {
                let mut radius = selected_style
                    .as_ref()
                    .map_or(self.radius, |style| style.radius);
                let radius_response =
                    ui.add(egui::Slider::new(&mut radius, 0..=16).text("Corner radius"));
                if radius_response.changed() {
                    self.radius = radius;
                    if self.tool == "select" {
                        if let Some((layer_index, object_index)) = self.selected {
                            self.save_history();
                            if let Some(object) = self
                                .doc
                                .layers
                                .get_mut(layer_index)
                                .and_then(|layer| layer.objects.get_mut(object_index))
                            {
                                object.style.radius = radius;
                                self.status = "Corner radius changed".into();
                            }
                        }
                    }
                } else if self.tool != "select" {
                    self.radius = radius;
                }
            }
            egui::ComboBox::from_id_salt("rounding")
                .selected_text(&self.rounding)
                .show_ui(ui, |ui| {
                    for mode in ["floor", "ceil", "nearest", "subpixel"] {
                        ui.selectable_value(&mut self.rounding, mode.into(), mode);
                    }
                });
            ui.separator();
            if ui.button("Finalize").clicked() {
                self.commit_pending(self.current_tool_kind() == "polygon");
            }
            if ui.button("Cancel").clicked() {
                self.pending.clear();
            }
            ui.separator();
            ui.separator();
            ui.label(&self.status);
            ui.label("Space + left-drag: pan");
            ui.label("Middle-drag: pan");
            ui.label("Wheel: zoom");
            ui.label("Right-click/Enter: finalize");
            if self.tool == "select" {
                ui.label("Hover control point: drag");
            }
        });
        egui::Panel::right("preview")
            .resizable(false)
            .default_size(config::ui::PREVIEW_PANEL_WIDTH)
            .show(ui, |ui| {
                ui.heading("1-bit Preview");
                self.preview(ui);
                ui.horizontal(|ui| {
                    if ui.button("Copy Playdate Lua").clicked() {
                        ui.ctx()
                            .copy_text(self.playdate_lua(false, self.doc.offset));
                        self.status = "Copied Playdate Lua".into();
                    }
                    ui.checkbox(&mut self.doc.offset, "offset");
                });
                if ui.button("Copy Anim Lua").clicked() {
                    ui.ctx().copy_text(self.playdate_lua(true, false));
                    self.status = "Copied Animation Lua".into();
                }
                ui.separator();
                ui.heading("Vectors");
                let mut reorder_request = None;
                let mut drag_target = None;
                let mut drag_stopped = false;
                let mut visibility_toggle = None;
                egui::ScrollArea::vertical()
                    .min_scrolled_height(config::ui::VECTOR_LIST_MIN_HEIGHT)
                    .max_height(config::ui::VECTOR_LIST_MIN_HEIGHT)
                    .show(ui, |ui| {
                        let vector_rows: Vec<(usize, String, VectorObject)> = self.doc.layers
                            [self.current_layer]
                            .objects
                            .iter()
                            .enumerate()
                            .map(|(index, object)| {
                                (
                                    index,
                                    format!("{}: {}", index + 1, object.kind),
                                    object.clone(),
                                )
                            })
                            .collect();
                        for (index, name, object) in vector_rows {
                            let is_selected = self.selected_objects.contains(&index);
                            let (shift_selection, command_selection) =
                                ui.input(|input| (input.modifiers.shift, input.modifiers.command));
                            let mut clicked = false;
                            let row_response = ui
                                .horizontal(|ui| {
                                    let drag_response = ui.add_sized(
                                        [
                                            config::ui::VECTOR_DRAG_HANDLE_WIDTH,
                                            config::ui::VECTOR_ROW_HEIGHT,
                                        ],
                                        egui::Label::new("≡").selectable(false),
                                    );
                                    drag_response.clone().on_hover_text("Drag to reorder");
                                    if Self::vector_visibility_button(ui, object.visible).clicked()
                                    {
                                        visibility_toggle = Some(index);
                                    }
                                    Self::vector_row_icon(ui, &object, is_selected);
                                    if ui.selectable_label(is_selected, name).clicked() {
                                        clicked = true;
                                    }
                                    if ui.small_button("↑").on_hover_text("Move up").clicked()
                                        && index > 0
                                    {
                                        reorder_request = Some((index, index - 1));
                                    }
                                    let object_count =
                                        self.doc.layers[self.current_layer].objects.len();
                                    if ui.small_button("↓").on_hover_text("Move down").clicked()
                                        && index + 1 < object_count
                                    {
                                        reorder_request = Some((index, index + 1));
                                    }
                                })
                                .response;
                            let row_left = row_response.rect.left();
                            let row_top = row_response.rect.top();
                            let row_bottom = row_response.rect.bottom();
                            let handle_rect = egui::Rect::from_min_max(
                                egui::pos2(row_left, row_top),
                                egui::pos2(
                                    row_left + config::ui::VECTOR_DRAG_HANDLE_WIDTH,
                                    row_bottom,
                                ),
                            );
                            let content_rect = egui::Rect::from_min_max(
                                egui::pos2(
                                    row_left
                                        + config::ui::VECTOR_DRAG_HANDLE_WIDTH
                                        + config::ui::VECTOR_VISIBILITY_WIDTH,
                                    row_top,
                                ),
                                egui::pos2(
                                    (row_response.rect.right()
                                        - config::ui::VECTOR_ROW_ACTION_WIDTH)
                                        .max(row_left),
                                    row_bottom,
                                ),
                            );
                            let handle_drag_response = ui.interact(
                                handle_rect,
                                ui.id().with(("vector_handle_drag", index)),
                                egui::Sense::click_and_drag(),
                            );
                            let content_drag_response = ui.interact(
                                content_rect,
                                ui.id().with(("vector_content_drag", index)),
                                egui::Sense::click_and_drag(),
                            );
                            if handle_drag_response.drag_started()
                                || content_drag_response.drag_started()
                            {
                                self.dragging_vector = Some(index);
                                self.select_single_object(index);
                                self.tool = "select".into();
                            }
                            if handle_drag_response.drag_stopped()
                                || content_drag_response.drag_stopped()
                            {
                                drag_stopped = true;
                            }
                            if self.dragging_vector.is_some()
                                && ui.input(|input| {
                                    input.pointer.latest_pos().is_some_and(|position| {
                                        row_response.rect.contains(position)
                                    })
                                })
                            {
                                drag_target = Some(index);
                            }
                            if clicked
                                || handle_drag_response.clicked()
                                || content_drag_response.clicked()
                            {
                                if shift_selection {
                                    self.select_object_range(index);
                                } else if command_selection {
                                    self.toggle_object_selection(index);
                                } else {
                                    self.select_single_object(index);
                                }
                                self.tool = "select".into();
                            }
                        }
                    });
                if drag_stopped || ui.input(|input| input.pointer.any_released()) {
                    if let (Some(from), Some(to)) = (self.dragging_vector.take(), drag_target) {
                        if from != to {
                            reorder_request = Some((from, to));
                        }
                    } else {
                        self.dragging_vector = None;
                    }
                }
                if let Some(object_index) = visibility_toggle {
                    self.save_history();
                    if let Some(object) = self
                        .doc
                        .layers
                        .get_mut(self.current_layer)
                        .and_then(|layer| layer.objects.get_mut(object_index))
                    {
                        object.visible = !object.visible;
                        self.status = if object.visible {
                            "Vector visible".into()
                        } else {
                            "Vector hidden".into()
                        };
                    }
                }
                if let Some((from, to)) = reorder_request {
                    self.reorder_vector(self.current_layer, from, to);
                }
                self.paint_vector_drag_preview(ui.ctx());
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        self.delete_selected();
                    }
                    if ui.button("Duplicate").clicked() {
                        self.duplicate_selected();
                    }
                });

                ui.separator();
                ui.heading("Control Points");
                let selected_points = if self.has_multiple_selected_objects() {
                    None
                } else {
                    self.selected
                        .and_then(|(layer_index, object_index)| {
                            self.doc
                                .layers
                                .get(layer_index)
                                .and_then(|layer| layer.objects.get(object_index))
                        })
                        .map(|object| object.points.clone())
                };

                let mut point_edits: Vec<(usize, [f32; 2])> = Vec::new();
                if let Some(points) = selected_points {
                    if points.is_empty() {
                        ui.label("制御点がありません");
                    } else {
                        for (index, point) in points.iter().enumerate() {
                            ui.horizontal(|ui| {
                                let is_selected_point = self.selected_point == Some(index);
                                if ui
                                    .selectable_label(is_selected_point, format!("P{}", index + 1))
                                    .clicked()
                                {
                                    self.selected_point = Some(index);
                                    self.tool = "select".into();
                                }

                                let mut x = point[0];
                                let mut y = point[1];
                                let x_changed = ui
                                    .add(egui::DragValue::new(&mut x).speed(0.1).prefix("x: "))
                                    .changed();
                                let y_changed = ui
                                    .add(egui::DragValue::new(&mut y).speed(0.1).prefix("y: "))
                                    .changed();
                                if x_changed || y_changed {
                                    point_edits.push((index, [x, y]));
                                }
                            });
                        }
                    }
                } else {
                    if self.has_multiple_selected_objects() {
                        ui.label("複数選択中は制御点を表示しません");
                    } else {
                        ui.label("ベクターを選択してください");
                    }
                }

                if !point_edits.is_empty() {
                    self.save_history();
                    if let Some((layer_index, object_index)) = self.selected {
                        if let Some(object) = self
                            .doc
                            .layers
                            .get_mut(layer_index)
                            .and_then(|layer| layer.objects.get_mut(object_index))
                        {
                            for (index, point) in point_edits {
                                if let Some(target) = object.points.get_mut(index) {
                                    *target = point;
                                }
                            }
                            self.status = "Control point updated".into();
                        }
                    }
                }

                let selected_fill_state = self
                    .selected
                    .and_then(|(layer_index, object_index)| {
                        self.doc
                            .layers
                            .get(layer_index)
                            .and_then(|layer| layer.objects.get(object_index))
                    })
                    .map(|object| (object.kind.clone(), object.style.fill));
                if let Some((kind, mut fill)) = selected_fill_state {
                    if matches!(kind.as_str(), "rect" | "round_rect" | "ellipse" | "polygon") {
                        if ui.checkbox(&mut fill, "Fill").changed() {
                            if let Some((layer_index, object_index)) = self.selected {
                                self.save_history();
                                if let Some(object) = self
                                    .doc
                                    .layers
                                    .get_mut(layer_index)
                                    .and_then(|layer| layer.objects.get_mut(object_index))
                                {
                                    object.style.fill = fill;
                                    self.status = if fill {
                                        "Fill enabled".into()
                                    } else {
                                        "Fill disabled".into()
                                    };
                                }
                            }
                        }
                    }
                }
            });
        egui::containers::CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Undo").clicked() {
                    self.undo_document();
                }
                if ui.button("Redo").clicked() {
                    self.redo_document();
                }
                ui.separator();
                ui.label(format!("Zoom: {:.2}x", self.zoom));
                if ui.button("-").clicked() {
                    self.zoom = (self.zoom - 0.25).max(0.25);
                }
                if ui.button("+").clicked() {
                    self.zoom = (self.zoom + 0.25).min(self.max_zoom());
                }
                if ui.button("Reset").clicked() {
                    self.zoom = 2.0;
                    self.pan = Vec2::ZERO;
                }
                ui.separator();
                ui.checkbox(&mut self.pixel_preview, "Pixel preview");
            });
            ui.separator();
            self.draw_canvas(ui);
        });
        if self.close_dialog {
            egui::Window::new("Unsaved Changes")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label("There are unsaved changes. Save before quitting?");
                    ui.horizontal(|ui| {
                        if ui.button("Save & Quit").clicked() {
                            self.save_json_document();
                            if !self.document_is_dirty() {
                                self.allow_close = true;
                                self.close_dialog = false;
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                        if ui.button("Quit Without Saving").clicked() {
                            self.allow_close = true;
                            self.close_dialog = false;
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.button("Cancel").clicked() {
                            self.close_dialog = false;
                        }
                    });
                });
        }
        if self.resolution_dialog {
            let mut apply_resolution = None;
            egui::Window::new("Change Resolution")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label("Canvas resolution (8 px increments)");
                    ui.horizontal(|ui| {
                        ui.label("Width");
                        ui.add(
                            egui::DragValue::new(&mut self.resolution_width)
                                .range(8..=400)
                                .speed(8),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.resolution_width, 8..=400)
                                .step_by(8.0)
                                .show_value(false),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Height");
                        ui.add(
                            egui::DragValue::new(&mut self.resolution_height)
                                .range(8..=400)
                                .speed(8),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.resolution_height, 8..=400)
                                .step_by(8.0)
                                .show_value(false),
                        );
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Apply").clicked() {
                            let width =
                                ((self.resolution_width.clamp(8, 400) + 4) / 8 * 8).clamp(8, 400);
                            let height =
                                ((self.resolution_height.clamp(8, 400) + 4) / 8 * 8).clamp(8, 400);
                            apply_resolution = Some((width, height));
                        }
                        if ui.button("Cancel").clicked() {
                            self.resolution_dialog = false;
                        }
                    });
                });
            if let Some((width, height)) = apply_resolution {
                self.save_history();
                self.doc.target.width = width;
                self.doc.target.height = height;
                self.last_fitted_target = None;
                self.resolution_width = width;
                self.resolution_height = height;
                self.resolution_dialog = false;
                self.status = format!("Resolution changed: {} x {}", width, height);
            }
        }
        if self.new_dialog {
            let mut create_document = None;
            egui::Window::new("New Document")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label("Canvas resolution");
                    ui.horizontal(|ui| {
                        ui.label("Width");
                        ui.add(egui::TextEdit::singleline(&mut self.new_width).desired_width(70.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Height");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.new_height).desired_width(70.0),
                        );
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Create").clicked() {
                            let width = self.new_width.trim().parse::<i32>().ok();
                            let height = self.new_height.trim().parse::<i32>().ok();
                            if let (Some(width), Some(height)) = (width, height) {
                                if width > 0 && height > 0 {
                                    create_document = Some((width, height));
                                } else {
                                    self.status = "Resolution must be positive".into();
                                }
                            } else {
                                self.status = "Enter numeric width and height".into();
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.new_dialog = false;
                        }
                    });
                });
            if let Some((width, height)) = create_document {
                self.save_history();
                self.doc = Document::default();
                self.doc.target.width = width;
                self.doc.target.height = height;
                self.current_file = None;
                self.pending.clear();
                self.selected = None;
                self.selected_objects.clear();
                self.selected_point = None;
                self.current_layer = 0;
                self.new_dialog = false;
                self.status = format!("New document: {} x {}", width, height);
            }
        }
        if self.reference_window {
            let viewport_id = egui::ViewportId::from_hash_of("reference_preview");
            ui.ctx().show_viewport_immediate(
                viewport_id,
                egui::ViewportBuilder::default()
                    .with_title("Reference Preview")
                    .with_inner_size([720.0, 620.0])
                    .with_min_inner_size([360.0, 260.0]),
                |ui, _class| {
                    if ui.input(|input| input.viewport().close_requested()) {
                        self.reference_window = false;
                    }
                    self.draw_reference_preview(ui);
                },
            );
            if self.reference_focus_requested {
                ui.ctx()
                    .send_viewport_cmd_to(viewport_id, egui::ViewportCommand::Focus);
                self.reference_focus_requested = false;
            }
        }
        self.update_window_title(ui.ctx());
    }
}
