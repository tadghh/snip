#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use eframe::egui::{self, pos2};

use overlay::SnipOverlay;
use screenshots::Screen;

mod overlay;
mod util;

fn main() -> Result<(), eframe::Error> {
    println!("Starting Snip & Sketch (Alt) - press Ctrl+Shift+S to capture");

    take_screenshot()
}

fn take_screenshot() -> Result<(), eframe::Error> {
    let screens = Screen::all().unwrap();
    println!("Detected {} screens", screens.len());

    // Fast path for single screen
    if screens.len() == 1 {
        let image = screens[0].capture().unwrap();
        return run_selection_overlay(image.buffer().to_vec(), image.width(), image.height(), 0);
    }

    // Calculate bounding box and track min height for all screens
    let (min_x, min_y, max_x, max_y, min_height) = screens.iter().fold(
        (i32::MAX, i32::MAX, i32::MIN, i32::MIN, u32::MAX),
        |(min_x, min_y, max_x, max_y, min_height), screen| {
            let x = screen.display_info.x;
            let y = screen.display_info.y;
            let width = screen.display_info.width as i32;
            let height = screen.display_info.height;

            println!(
                "Screen at position ({}, {}) with dimensions: {}x{}",
                x, y, width, height
            );

            (
                min_x.min(x),
                min_y.min(y),
                max_x.max(x + width),
                max_y.max(y + height as i32),
                min_height.min(height),
            )
        },
    );

    let total_width = (max_x - min_x) as u32;
    let total_height = (max_y - min_y) as u32;
    println!(
        "Combined dimensions: {}x{} (from {}:{} to {}:{})",
        total_width, total_height, min_x, min_y, max_x, max_y
    );

    // Pre-allocate combined buffer
    let mut combined_buffer = vec![0u8; (total_width * total_height * 4) as usize];

    // Copy each screen's contents to the combined buffer
    for (i, screen) in screens.iter().enumerate() {
        let screenshot = match screen.capture() {
            Ok(s) => s,
            Err(_) => continue,
        };

        let buffer = screenshot.buffer();
        let width = screen.display_info.width;
        let height = screen.display_info.height;
        let pos_x = (screen.display_info.x - min_x) as u32;
        let pos_y = (screen.display_info.y - min_y) as u32;

        println!(
            "Processing screen {}: {}x{} at position ({}, {})",
            i, width, height, pos_x, pos_y
        );

        // Load the image data
        let image = match image::load_from_memory(&buffer) {
            Ok(img) => img.to_rgba8(),
            Err(_) => continue,
        };
        let image_data = image.into_vec();

        // Copy pixels in a single loop with bounds checking
        for y_offset in 0..height {
            let src_row_start = (y_offset * width * 4) as usize;
            let dst_row_start = ((pos_y + y_offset) * total_width * 4) as usize;

            for x_offset in 0..width {
                let src_idx = src_row_start + (x_offset * 4) as usize;
                let dst_idx = dst_row_start + ((pos_x + x_offset) * 4) as usize;

                // Skip out-of-bounds pixels
                if src_idx + 3 >= image_data.len() || dst_idx + 3 >= combined_buffer.len() {
                    continue;
                }

                // Copy RGBA values in one go
                combined_buffer[dst_idx..dst_idx + 4]
                    .copy_from_slice(&image_data[src_idx..src_idx + 4]);
            }
        }
    }

    let width_offset = min_x.abs() as u32;
    let height_offset = min_y.abs() as u32;
    run_selection_overlay(
        combined_buffer,
        total_width,
        total_height,
        width_offset,
        height_offset,
    )
}

fn run_selection_overlay(
    screenshot_data: Vec<u8>,
    width: u32,
    height: u32,
    width_offset: u32,
    height_offset: u32,
) -> Result<(), eframe::Error> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_min_inner_size([width as f32, height as f32])
            .with_transparent(true)
            .with_decorations(false)
            .with_active(true)
            .with_window_level(egui::WindowLevel::AlwaysOnTop)
            .with_position(pos2(
                width_offset as f32 * -1.0,
                height_offset as f32 * -1.0,
            ))
            .with_taskbar(false),
        run_and_return: true,
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "Snip Overlay",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(SnipOverlay::new(
                cc,
                screenshot_data,
                width,
                height,
            )))
        }),
    )
}
