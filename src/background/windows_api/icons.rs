//! Icon extraction through `IShellItemImageFactory`, which handles executables,
//! shortcuts, folders, documents and packaged apps (`shell:AppsFolder\<umid>`) alike.

use windows::{
    Win32::{
        Foundation::SIZE,
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, DeleteObject, GetDC, GetDIBits,
            HBITMAP, ReleaseDC,
        },
        UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_ICONONLY},
    },
    core::HSTRING,
};

use super::ComGuard;
use crate::error::Result;

/// Large enough for magnified dock icons and the launcher grid on high-DPI screens.
pub const ICON_SIZE: i32 = 128;

/// Returns PNG encoded bytes of the icon for a parsing name (path or shell uri).
pub fn extract_png(parsing_name: &str) -> Result<Vec<u8>> {
    let _com = ComGuard::new();
    let factory: IShellItemImageFactory =
        unsafe { SHCreateItemFromParsingName(&HSTRING::from(parsing_name), None)? };
    let bitmap = unsafe {
        factory.GetImage(
            SIZE {
                cx: ICON_SIZE,
                cy: ICON_SIZE,
            },
            SIIGBF_ICONONLY,
        )?
    };
    let result = bitmap_to_png(bitmap);
    unsafe {
        let _ = DeleteObject(bitmap.into());
    }
    result
}

fn bitmap_to_png(bitmap: HBITMAP) -> Result<Vec<u8>> {
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            ..Default::default()
        },
        ..Default::default()
    };
    let hdc = unsafe { GetDC(None) };
    // first call fills the header with the bitmap dimensions
    let ok = unsafe { GetDIBits(hdc, bitmap, 0, 0, None, &mut info, DIB_RGB_COLORS) };
    if ok == 0 {
        unsafe { ReleaseDC(None, hdc) };
        return Err("GetDIBits failed".into());
    }
    let width = info.bmiHeader.biWidth;
    let height = info.bmiHeader.biHeight.abs();
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB.0;
    info.bmiHeader.biHeight = -height; // top-down rows
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let lines = unsafe {
        GetDIBits(
            hdc,
            bitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut info,
            DIB_RGB_COLORS,
        )
    };
    unsafe { ReleaseDC(None, hdc) };
    if lines == 0 {
        return Err("GetDIBits failed".into());
    }

    // BGRA (premultiplied) -> RGBA (straight)
    let has_alpha = pixels.as_chunks::<4>().0.iter().any(|p| p[3] != 0);
    for px in pixels.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
        if !has_alpha {
            px[3] = 255;
        } else if px[3] != 0 && px[3] != 255 {
            let a = px[3] as u32;
            for c in &mut px[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }

    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width as u32, height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("png encode: {e}"))?;
        writer
            .write_image_data(&pixels)
            .map_err(|e| format!("png encode: {e}"))?;
    }
    Ok(out)
}
