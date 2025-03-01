use core::ptr::copy_nonoverlapping;
use device_query::{DeviceQuery, DeviceState, Keycode};
use eframe::egui::pos2;
use eframe::egui::{self};
use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use screenshots::Screen;
use std::os::windows::ffi::OsStrExt;
use std::thread;
use std::time::Duration;
use windows::Win32::Foundation::{BOOL, GetLastError, HANDLE, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GMEM_ZEROINIT, GlobalAlloc, GlobalFree, GlobalLock, GlobalUnlock,
};
use windows::Win32::System::Ole::CF_HDROP;
use windows::Win32::UI::Shell::DROPFILES;

const WINDOW_TRANSPARENCY: u8 = 180;
const ROUNDING: f32 = 0.5;
static mut SNIPPED: bool = false;
fn main() -> Result<(), eframe::Error> {
    println!("Starting Snip & Sketch (Alt) - press Ctrl+Shift+S to capture");

    let device_state = DeviceState::new();

    loop {
        let keys: Vec<Keycode> = device_state.get_keys();

        if keys.contains(&Keycode::LShift)
            && keys.contains(&Keycode::LControl)
            && keys.contains(&Keycode::S)
        {
            println!("Detected Ctrl+Shift+S, taking screenshot");
            return take_screenshot();
        }

        thread::sleep(Duration::from_millis(100));
    }
}

fn take_screenshot() -> Result<(), eframe::Error> {
    let screens = Screen::all().unwrap();
    // TODO this only gets the main screen atm
    let image = screens.get(1).unwrap().capture().unwrap();
    let buffer = image.buffer();
    let width = image.width();
    let height = image.height();

    run_selection_overlay(buffer.to_vec(), width, height)
}

fn run_selection_overlay(
    screenshot_data: Vec<u8>,
    width: u32,
    height: u32,
) -> Result<(), eframe::Error> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_min_inner_size([width as f32, height as f32])
            .with_transparent(true)
            .with_decorations(false)
            .with_active(true)
            // TODO need to add negative offset depending on multi monitor setup
            .with_position(pos2(0.0, 0.0))
            .with_taskbar(false),
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "Snip Overlay",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(SnipOverlay::new(
                cc,
                &screenshot_data,
                width,
                height,
            )))
        }),
    )
}

#[derive(Default)]
struct SnipOverlay {
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
        if self.selection_complete && self.selected_rect.is_some() && unsafe { !SNIPPED } {
            unsafe {
                SNIPPED = true;
            }
            let rect = self.selected_rect.unwrap();
            println!("Selection completed: {:?}, copying to clipboard", rect);
            copy_selection_to_clipboard(&self.screenshot_data, self.width, self.height, rect);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
    }
}

fn copy_selection_to_clipboard(
    screenshot_data: &[u8],
    screen_width: u32,
    screen_height: u32,
    rect: Rect,
) {
    let x = rect.min.x.max(0.0) as u32;
    let y = rect.min.y.max(0.0) as u32;
    let width = rect.width() as u32;
    let height = rect.height() as u32;
    let width = width.min(screen_width - x);
    let height = height.min(screen_height - y);

    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join(format!("screenshot_{}.png", "snip"));

    let img = match image::load_from_memory(screenshot_data) {
        Ok(img) => img,
        Err(err) => {
            println!("{:?}", err);
            return;
        }
    };

    // TODO add image properties date time
    let cropped = img.crop_imm(x, y, width, height);

    match cropped.save(&temp_path) {
        Ok(_) => {}
        Err(err) => {
            println!("{:?}", err);
            return;
        }
    };

    let abs_path = temp_path;

    if !abs_path.exists() {
        println!(
            "Warning: File was not found at path: {}",
            abs_path.display()
        );
        return;
    }

    unsafe {
        let dropfiles_size = size_of::<DROPFILES>();

        let mut wide_path: Vec<u16> = abs_path.as_os_str().encode_wide().collect();
        wide_path.push(0);

        let path_size = wide_path.len() * size_of::<u16>();
        let total_size = dropfiles_size + path_size + size_of::<u16>();

        if !OpenClipboard(HWND(0)).as_bool() {
            let error = GetLastError();
            println!("Failed to open clipboard. Error code: {}", error.0);
            return;
        }

        if !EmptyClipboard().as_bool() {
            let error = GetLastError();
            println!("Failed to empty clipboard. Error code: {}", error.0);
            CloseClipboard();
            return;
        }

        let h_glob = match GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, total_size) {
            Ok(handle) => handle,
            Err(e) => {
                println!("Failed to allocate global memory: {:?}", e);
                CloseClipboard();
                return;
            }
        };

        let p_glob = GlobalLock(h_glob);

        if !p_glob.is_null() {
            let p_drop = p_glob as *mut DROPFILES;
            (*p_drop).pFiles = dropfiles_size as u32;
            (*p_drop).pt.x = 0;
            (*p_drop).pt.y = 0;
            (*p_drop).fNC = BOOL(0);
            (*p_drop).fWide = BOOL(1);

            let p_path = (p_glob as usize + dropfiles_size) as *mut u16;
            copy_nonoverlapping(wide_path.as_ptr(), p_path, wide_path.len());

            *p_path.add(wide_path.len() - 1) = 0;

            GlobalUnlock(h_glob);

            let h_drop = SetClipboardData(CF_HDROP.0 as u32, HANDLE(h_glob.0 as isize));

            if h_drop.unwrap().is_invalid() {
                let error = GetLastError();
                println!("Failed to set clipboard data. Error code: {}", error.0);
                GlobalFree(h_glob).ok();
                CloseClipboard();
                return;
            }
        } else {
            let error = GetLastError();
            println!("Failed to lock global memory. Error code: {}", error.0);
            GlobalFree(h_glob).ok();
            CloseClipboard();
            return;
        }

        if !CloseClipboard().as_bool() {
            let error = GetLastError();
            println!("Failed to close clipboard. Error code: {}", error.0);
        }
    }
    println!(
        "Image saved and file reference copied to clipboard: {}",
        abs_path.display()
    );
}
