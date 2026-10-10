//! Icon extraction through `IShellItemImageFactory`, which handles executables,
//! shortcuts, folders, documents and packaged apps (`shell:AppsFolder\<umid>`) alike.

use windows::{
    Win32::{
        Foundation::SIZE,
        Graphics::Gdi::{
            BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection,
            DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, GetObjectW, HBITMAP,
            ReleaseDC, SelectObject,
        },
        UI::{
            Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_ICONONLY},
            WindowsAndMessaging::{
                DI_FLAGS, DI_MASK, DI_NORMAL, DrawIconEx, GetIconInfo, HICON, ICONINFO,
            },
        },
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

    encode_png(width as u32, height as u32, &pixels)
}

/// Draws `icon` at `size` px into a 32-bit top-down DIB and returns its BGRA pixels.
fn draw_icon(icon: HICON, size: i32, flags: DI_FLAGS) -> Result<Vec<u8>> {
    unsafe {
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size,
                biHeight: -size, // top-down rows
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let result = match CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(bitmap) if !bits.is_null() => {
                let old = SelectObject(dc, bitmap.into());
                let drawn = DrawIconEx(dc, 0, 0, icon, size, size, 0, None, flags);
                let len = (size * size * 4) as usize;
                let pixels = std::slice::from_raw_parts(bits as *const u8, len).to_vec();
                SelectObject(dc, old);
                let _ = DeleteObject(bitmap.into());
                drawn.map(|_| pixels).map_err(Into::into)
            }
            Ok(bitmap) => {
                let _ = DeleteObject(bitmap.into());
                Err("CreateDIBSection returned no pixels".into())
            }
            Err(err) => Err(err.into()),
        };
        let _ = DeleteDC(dc);
        ReleaseDC(None, screen);
        result
    }
}

/// Width in px of the image an icon handle holds.
fn icon_size(icon: HICON) -> Option<i32> {
    unsafe {
        let mut info = ICONINFO::default();
        GetIconInfo(icon, &mut info).ok()?;
        // monochrome icons have no color bitmap (their mask holds both images)
        let source = if info.hbmColor.is_invalid() {
            info.hbmMask
        } else {
            info.hbmColor
        };
        let mut bitmap = BITMAP::default();
        let read = GetObjectW(
            source.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bitmap as *mut BITMAP as *mut _),
        );
        // GetIconInfo hands out copies of both bitmaps
        for copy in [info.hbmColor, info.hbmMask] {
            if !copy.is_invalid() {
                let _ = DeleteObject(copy.into());
            }
        }
        (read != 0).then_some(bitmap.bmWidth)
    }
}

/// PNG of an icon handle (notification area icons) at the size the application made it
/// (24 px on a 150% display), so it is shown pixel for pixel like on the taskbar instead
/// of being stretched and scaled back; `fallback` px square when the size is unknown.
pub fn hicon_to_png(icon: HICON, fallback: i32) -> Result<Vec<u8>> {
    let size = icon_size(icon)
        .filter(|s| (8..=256).contains(s))
        .unwrap_or(fallback);
    let mut pixels = draw_icon(icon, size, DI_NORMAL)?;
    let has_alpha = pixels.as_chunks::<4>().0.iter().any(|p| p[3] != 0);
    // icons without an alpha channel: transparency comes from the AND mask
    let mask = if has_alpha {
        None
    } else {
        Some(draw_icon(icon, size, DI_MASK)?)
    };
    for (i, px) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        px.swap(0, 2);
        if let Some(mask) = &mask {
            // white in the mask = transparent
            px[3] = if mask.get(i * 4).copied().unwrap_or(0) > 127 {
                0
            } else {
                255
            };
        } else if px[3] != 0 && px[3] != 255 {
            let a = px[3] as u32;
            for c in &mut px[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    encode_png(size as u32, size as u32, &pixels)
}

/// RGBA (straight alpha) -> PNG bytes.
fn encode_png(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("png encode: {e}"))?;
        writer
            .write_image_data(pixels)
            .map_err(|e| format!("png encode: {e}"))?;
    }
    Ok(out)
}
