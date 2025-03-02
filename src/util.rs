use chrono::Local;
use core::ptr::copy_nonoverlapping;
use eframe::egui::{self};
use egui::Rect;
use image::{DynamicImage, RgbaImage};
use std::env::temp_dir;
use std::fs::{read_dir, remove_file};
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use windows::Win32::Foundation::{BOOL, GetLastError, HANDLE, HWND};
use windows::Win32::System::{
    DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
    Memory::{GMEM_MOVEABLE, GMEM_ZEROINIT, GlobalAlloc, GlobalFree, GlobalLock, GlobalUnlock},
    Ole::CF_HDROP,
};
use windows::Win32::UI::Shell::DROPFILES;

pub fn copy_selection_to_clipboard(
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

    let temp_dir = temp_dir();
    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let temp_path = temp_dir.join(format!("screenshot_snip_{}.png", timestamp));
    let abs_path = &temp_path;
    let current_filename = abs_path.file_name().unwrap();

    let img = match RgbaImage::from_raw(screen_width, screen_height, screenshot_data.to_vec()) {
        Some(img) => DynamicImage::ImageRgba8(img),
        None => {
            println!("Failed to create image from raw data");
            return;
        }
    };

    let cropped = img.crop_imm(x, y, width, height);

    match cropped.save(&temp_path) {
        Ok(_) => {}
        Err(err) => {
            println!("{:?}", err);
            return;
        }
    };

    if !abs_path.exists() {
        println!(
            "Warning: File was not found at path: {}",
            abs_path.display()
        );
    }

    unsafe {
        let dropfiles_size = size_of::<DROPFILES>();

        let mut wide_path: Vec<u16> = abs_path.as_os_str().encode_wide().collect();
        wide_path.push(0);

        let path_size = wide_path.len() * size_of::<u16>();
        let total_size = dropfiles_size + path_size + size_of::<u16>();

        if !OpenClipboard(HWND(0)).as_bool() {
            println!("Failed to open clipboard. Error code: {}", GetLastError().0);
            return;
        }

        if !EmptyClipboard().as_bool() {
            println!(
                "Failed to empty clipboard. Error code: {}",
                GetLastError().0
            );
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
                println!(
                    "Failed to set clipboard data. Error code: {}",
                    GetLastError().0
                );
                GlobalFree(h_glob).ok();
                CloseClipboard();
                return;
            }
        } else {
            println!(
                "Failed to lock global memory. Error code: {}",
                GetLastError().0
            );
            GlobalFree(h_glob).ok();
            CloseClipboard();
            return;
        }

        if !CloseClipboard().as_bool() {
            println!(
                "Failed to close clipboard. Error code: {}",
                GetLastError().0
            );
        }
    }
    println!(
        "Image saved and file reference copied to clipboard: {}",
        abs_path.display()
    );

    if let Ok(entries) = read_dir(&temp_dir) {
        use rayon::prelude::*;

        let entry_paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();

        // We are just going to assume the user never emptied their temp folder
        entry_paths
            .into_par_iter()
            .filter(|file_path| {
                if let Some(filename) = file_path.file_name() {
                    if filename == current_filename {
                        return false;
                    }

                    if let Some(filename_str) = filename.to_str() {
                        return filename_str.starts_with("screenshot_snip")
                            && filename_str.ends_with(".png");
                    }
                }
                false
            })
            .for_each(|file_path| {
                if let Err(e) = remove_file(&file_path) {
                    println!(
                        "Failed to delete old screenshot file {}: {}",
                        file_path.display(),
                        e
                    );
                }
            });
    }
}
