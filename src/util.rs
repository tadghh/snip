use core::ptr::copy_nonoverlapping;
use eframe::egui::{self};
use egui::Rect;
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

    let temp_dir = std::env::temp_dir();
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let temp_path = temp_dir.join(format!("screenshot_{}.png", timestamp));

    if width == 0 || height == 0 {
        return;
    }

    let img =
        match image::RgbaImage::from_raw(screen_width, screen_height, screenshot_data.to_vec()) {
            Some(img) => image::DynamicImage::ImageRgba8(img),
            None => {
                println!("Failed to create image from raw data");
                return;
            }
        };

    let cropped = img.crop_imm(x, y, width, height);

    match cropped.save(&temp_path) {
        Ok(_) => {}
        Err(_) => return,
    };

    let abs_path = temp_path;

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
