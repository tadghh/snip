use eframe::egui::pos2;
use eframe::egui::{self};
use egui::{Color32, Pos2, Rect, Stroke, Vec2};

use crate::util::copy_selection_to_clipboard;

static mut SNIPPING: bool = false;
const WINDOW_TRANSPARENCY: u8 = 180;
const ROUNDING: f32 = 0.5;

#[derive(Default)]
pub struct SnipOverlay {
    screenshot_data: Vec<u8>,
    width: u32,
    height: u32,
    start_pos: Option<Pos2>,
    current_pos: Option<Pos2>,
    selection_complete: bool,
    selected_rect: Option<Rect>,
}

impl SnipOverlay {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        screenshot_data: &[u8],
        width: u32,
        height: u32,
    ) -> Self {
        egui::ViewportCommand::center_on_screen(&cc.egui_ctx);
        cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        Self {
            screenshot_data: screenshot_data.to_vec(),
            width,
            height,
            start_pos: None,
            current_pos: None,
            selection_complete: false,
            selected_rect: None,
        }
    }
}

impl eframe::App for SnipOverlay {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        unsafe {
            if self.selection_complete && self.selected_rect.is_some() && !SNIPPING {
                SNIPPING = true;
                let rect = self.selected_rect.unwrap();
                println!("Selection completed: {:?}, copying to clipboard", rect);
                copy_selection_to_clipboard(&self.screenshot_data, self.width, self.height, rect);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let screen_rect = egui::Rect::from_min_size(
                    Pos2::ZERO,
                    egui::vec2(self.width as f32, self.height as f32),
                );

                let response = ui.allocate_rect(screen_rect, egui::Sense::drag());

                if response.drag_started() {
                    println!("Selection started");
                    self.start_pos = response.hover_pos();
                }

                if response.dragged() {
                    self.current_pos = response.hover_pos();
                }

                if response.drag_stopped() && self.start_pos.is_some() && self.current_pos.is_some()
                {
                    let start = self.start_pos.unwrap();
                    let end = self.current_pos.unwrap();
                    let rect = Rect::from_two_pos(start, end);

                    println!("Selection released: {:?}", rect);
                    self.selected_rect = Some(rect);
                    self.selection_complete = true;
                }

                if let (Some(start), Some(current)) = (self.start_pos, self.current_pos) {
                    let selection_rect = Rect::from_two_pos(start, current);

                    let top_rect = Rect::from_min_max(
                        screen_rect.min,
                        pos2(screen_rect.max.x, selection_rect.min.y),
                    );

                    let bottom_rect = Rect::from_min_max(
                        pos2(screen_rect.min.x, selection_rect.max.y),
                        screen_rect.max,
                    );

                    let left_rect = Rect::from_min_max(
                        pos2(screen_rect.min.x, selection_rect.min.y),
                        pos2(selection_rect.min.x, selection_rect.max.y),
                    );

                    let right_rect = Rect::from_min_max(
                        pos2(selection_rect.max.x, selection_rect.min.y),
                        pos2(screen_rect.max.x, selection_rect.max.y),
                    );

                    let overlay_color =
                        Color32::from_rgba_unmultiplied(0, 0, 0, WINDOW_TRANSPARENCY);
                    ui.painter().rect_filled(top_rect, ROUNDING, overlay_color);
                    ui.painter()
                        .rect_filled(bottom_rect, ROUNDING, overlay_color);
                    ui.painter().rect_filled(left_rect, ROUNDING, overlay_color);
                    ui.painter()
                        .rect_filled(right_rect, ROUNDING, overlay_color);

                    ui.painter().rect_stroke(
                        selection_rect,
                        ROUNDING,
                        Stroke::new(2.0, Color32::from_rgb(255, 105, WINDOW_TRANSPARENCY)),
                    );

                    let dimensions = format!(
                        "{}x{}",
                        selection_rect.width() as i32,
                        selection_rect.height() as i32
                    );
                    let text_pos = selection_rect.min - Vec2::new(0.0, 2.5);

                    ui.painter().text(
                        text_pos,
                        egui::Align2::LEFT_BOTTOM,
                        dimensions,
                        egui::FontId::proportional(14.0),
                        Color32::WHITE,
                    );
                } else {
                    ui.painter().rect_filled(
                        screen_rect,
                        0.0,
                        Color32::from_rgba_unmultiplied(0, 0, 0, WINDOW_TRANSPARENCY),
                    );
                }
            });

        // Keep UI responsive
        ctx.request_repaint();
    }
}
