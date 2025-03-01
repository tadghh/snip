#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use eframe::egui::pos2;
use eframe::egui::{self};

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

    if screens.len() == 1 {
        let image = screens[0].capture().unwrap();
        let buffer = image.buffer();
        let width = image.width();
        let height = image.height();

        return run_selection_overlay(buffer.to_vec(), width, height, 0);
    }

    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for screen in &screens {
        let x = screen.display_info.x;
        let y = screen.display_info.y;
        let width = screen.display_info.width;
        let height = screen.display_info.height;

        println!(
            "Screen at position ({}, {}) with dimensions: {}x{}",
            x, y, width, height
        );

        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x + width as i32);
        max_y = max_y.max(y + height as i32);
    }

    let total_width = (max_x - min_x) as u32;
    let total_height = (max_y - min_y) as u32;
    println!(
        "Combined dimensions: {}x{} (from {}:{} to {}:{})",
        total_width, total_height, min_x, min_y, max_x, max_y
    );

    let mut combined_buffer = vec![0u8; (total_width * total_height * 4) as usize];

    for (i, screen) in screens.iter().enumerate() {
        let x = screen.display_info.x;
        let y = screen.display_info.y;
        let width = screen.display_info.width;
        let height = screen.display_info.height;

        let pos_x = (x - min_x) as u32;
        let pos_y = (y - min_y) as u32;

        let screenshot = screen.capture().unwrap();
        let buffer = screenshot.buffer();

        println!(
            "Processing screen {}: {}x{} at position ({}, {})",
            i, width, height, pos_x, pos_y
        );
        println!(
            "Buffer size: {}, Expected: {}",
            buffer.len(),
            (width * height * 4) as usize
        );

        let image = if buffer.len() > 8 && buffer.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
            match image::load_from_memory_with_format(&buffer, image::ImageFormat::Png) {
                Ok(img) => img.to_rgba8(),
                Err(_) => continue, // Skip this screen if we can't decode it
            }
        // }
        // else if buffer.len() > 3 && buffer.starts_with(&[0xFF, 0xD8, 0xFF]) {
        //     // JPEG format
        //     match image::load_from_memory_with_format(&buffer, image::ImageFormat::Jpeg) {
        //         Ok(img) => img.to_rgba8(),
        //         Err(_) => continue, // Skip this screen if we can't decode it
        //     }
        } else {
            // Generic format detection
            match image::load_from_memory(&buffer) {
                Ok(img) => img.to_rgba8(),
                Err(_) => continue, // Skip this screen if we can't decode it
            }
        };

        let image_data = image.into_vec();

        for y_offset in 0..height {
            for x_offset in 0..width {
                let src_idx = ((y_offset * width + x_offset) * 4) as usize;

                if src_idx + 3 >= image_data.len() {
                    continue;
                }

                let dst_x = pos_x + x_offset;
                let dst_y = pos_y + y_offset;
                let dst_idx = ((dst_y * total_width + dst_x) * 4) as usize;

                if dst_idx + 3 >= combined_buffer.len() {
                    continue;
                }

                combined_buffer[dst_idx] = image_data[src_idx]; // R
                combined_buffer[dst_idx + 1] = image_data[src_idx + 1]; // G
                combined_buffer[dst_idx + 2] = image_data[src_idx + 2]; // B
                combined_buffer[dst_idx + 3] = image_data[src_idx + 3]; // A
            }
        }
    }

    let mut min_height = u32::MAX;
    let max_height = total_height;
    for screen in &screens {
        if screen.display_info.height < min_height {
            min_height = screen.display_info.height;
        }
    }
    let height_adjustment = max_height - min_height;

    run_selection_overlay(
        combined_buffer,
        total_width,
        total_height,
        height_adjustment,
    )
}

fn run_selection_overlay(
    screenshot_data: Vec<u8>,
    width: u32,
    height: u32,
    height_offset: u32,
) -> Result<(), eframe::Error> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_min_inner_size([width as f32, height as f32])
            .with_transparent(true)
            .with_decorations(false)
            .with_active(true)
            .with_position(pos2(0.0, height_offset as f32 * -1.0))
            .with_taskbar(false),
        run_and_return: true,
        persist_window: false,
        ..Default::default()
    };

    let screenshot_data_clone = screenshot_data.clone();
    let width_clone = width;
    let height_clone = height;

    eframe::run_native(
        "Snip Overlay",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(SnipOverlay::new(
                cc,
                &screenshot_data_clone,
                width_clone,
                height_clone,
            )))
        }),
    )
}
