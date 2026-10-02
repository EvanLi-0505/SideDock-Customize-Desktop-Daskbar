//! Real blur behind the bar for the frosted glass theme.
//!
//! A transparent webview cannot blur what is behind its window, and giving the dock
//! window itself a native backdrop would blur the whole window (which is much larger than
//! the bar, see `mod.rs`). So every dock gets a small native window right below it in the
//! z-order, shaped like the bar. The page reports the bar shape (including the growth of
//! the magnification wave) and the backend pointer loop calls [`sync`] every frame to keep
//! the window in place and hide it with the dock.
//!
//! The blur is drawn with Windows.UI.Composition: the system's blurred "host backdrop"
//! clipped to a rounded rectangle, so the corner radius matches the bar exactly and is
//! anti-aliased. Where that is not available (Windows 10) the window falls back to the
//! acrylic blur-behind accent, which DWM can only round with its own fixed radius.
//!
//! The window sits exactly below the bar, where the dock window itself takes the mouse,
//! and answers `HTTRANSPARENT` to hit-testing.

use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::c_void,
    sync::{
        LazyLock,
        mpsc::{self, Sender},
    },
};

use parking_lot::Mutex;
use serde::Deserialize;
use windows::{
    UI::{
        Color,
        Composition::{
            CompositionRoundedRectangleGeometry, Compositor, Desktop::DesktopWindowTarget,
            SpriteVisual,
        },
    },
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Dwm::{
            DWMWA_USE_HOSTBACKDROPBRUSH, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
            DWMWCP_ROUND, DwmSetWindowAttribute,
        },
        System::{
            Com::{COINIT_APARTMENTTHREADED, CoInitializeEx},
            LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW},
            WinRT::{
                Composition::ICompositorDesktopInterop, CreateDispatcherQueueController,
                DQTAT_COM_STA, DQTYPE_THREAD_CURRENT, DispatcherQueueOptions,
            },
        },
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, MSG, PostMessageW,
            PostQuitMessage, RegisterClassW, SW_HIDE, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE,
            SWP_SHOWWINDOW, SetWindowPos, ShowWindow, TranslateMessage, WM_APP, WM_CLOSE,
            WM_DESTROY, WM_NCHITTEST, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_NOREDIRECTIONBITMAP,
            WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
        },
    },
    core::{BOOL, Interface, PCSTR, w},
};

use windows_numerics::Vector2;

use super::DockState;
use crate::windows_api::window::Rect;

/// Bar shape reported by the page, in CSS px relative to the dock window.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackdropShape {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub radius: f64,
}

/// ~150ms: the page slides/fades the dock content in over 220ms
const SHOW_DELAY_FRAMES: u32 = 9;
/// frames between two z-order checks (~1s)
const REORDER_EVERY: u32 = 60;
/// wparam = width | height << 16, lparam = radius (physical px)
const WM_BACKDROP_SHAPE: u32 = WM_APP + 1;
/// wparam = ARGB tint
const WM_BACKDROP_TINT: u32 = WM_APP + 2;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// Windows.UI.Composition host backdrop, any corner radius
    Composition,
    /// acrylic accent, DWM corners
    Accent,
}

struct Backdrop {
    hwnd: isize,
    mode: Mode,
    rect: Rect,
    /// last (width, height, radius) sent to the window thread
    shape: (i32, i32, i32),
    /// frames since the dock became visible
    visible_frames: u32,
    shown: bool,
    tint: u32,
    frames: u32,
}

static BACKDROPS: LazyLock<Mutex<HashMap<String, Backdrop>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn hwnd(h: isize) -> HWND {
    HWND(h as *mut _)
}

// ---------------- composition (window thread only) ----------------

struct Scene {
    // the target must stay alive for the visuals to be shown
    _target: DesktopWindowTarget,
    root: SpriteVisual,
    tint: SpriteVisual,
    compositor: Compositor,
    geometry: CompositionRoundedRectangleGeometry,
}

thread_local! {
    static SCENE: RefCell<Option<Scene>> = const { RefCell::new(None) };
}

fn build_scene(h: HWND) -> windows::core::Result<Scene> {
    unsafe {
        let enable = BOOL::from(true);
        // fails before Windows 11: the host backdrop would stay empty
        DwmSetWindowAttribute(
            h,
            DWMWA_USE_HOSTBACKDROPBRUSH,
            &enable as *const _ as *const c_void,
            std::mem::size_of::<BOOL>() as u32,
        )?;
    }
    let compositor = Compositor::new()?;
    let interop: ICompositorDesktopInterop = compositor.cast()?;
    let target = unsafe { interop.CreateDesktopWindowTarget(h, false)? };

    let root = compositor.CreateSpriteVisual()?;
    root.SetBrush(&compositor.CreateHostBackdropBrush()?)?;
    let geometry = compositor.CreateRoundedRectangleGeometry()?;
    root.SetClip(&compositor.CreateGeometricClipWithGeometry(&geometry)?)?;

    let tint = compositor.CreateSpriteVisual()?;
    root.Children()?.InsertAtTop(&tint)?;
    target.SetRoot(&root)?;
    Ok(Scene {
        _target: target,
        root,
        tint,
        compositor,
        geometry,
    })
}

fn scene_shape(width: f32, height: f32, radius: f32) -> windows::core::Result<()> {
    SCENE.with_borrow(|scene| {
        let Some(s) = scene else {
            return Ok(());
        };
        let size = Vector2 {
            X: width,
            Y: height,
        };
        s.root.SetSize(size)?;
        s.tint.SetSize(size)?;
        s.geometry.SetSize(size)?;
        s.geometry.SetCornerRadius(Vector2 {
            X: radius,
            Y: radius,
        })
    })
}

fn scene_tint(argb: u32) -> windows::core::Result<()> {
    SCENE.with_borrow(|scene| {
        let Some(s) = scene else {
            return Ok(());
        };
        let color = Color {
            A: (argb >> 24) as u8,
            R: (argb >> 16) as u8,
            G: (argb >> 8) as u8,
            B: argb as u8,
        };
        s.tint
            .SetBrush(&s.compositor.CreateColorBrushWithColor(color)?)
    })
}

/// Light veil over the blurred backdrop (ARGB); the page draws the glass color on top.
fn composition_tint(dark: bool) -> u32 {
    if dark { 0x33_18_1a_1f } else { 0x1f_f2_f4_f8 }
}

// ---------------- acrylic accent fallback (undocumented, stable since Windows 10 1803) ---

#[repr(C)]
struct AccentPolicy {
    state: u32,
    flags: u32,
    /// AABBGGRR
    gradient: u32,
    animation: u32,
}

#[repr(C)]
struct CompositionData {
    attribute: u32,
    data: *mut c_void,
    size: usize,
}

type SetWindowCompositionAttribute = unsafe extern "system" fn(HWND, *mut CompositionData) -> i32;

const WCA_ACCENT_POLICY: u32 = 19;
const ACCENT_ENABLE_ACRYLICBLURBEHIND: u32 = 4;

static SET_COMPOSITION: LazyLock<Option<SetWindowCompositionAttribute>> =
    LazyLock::new(|| unsafe {
        let module = LoadLibraryW(w!("user32.dll")).ok()?;
        let proc = GetProcAddress(
            module,
            PCSTR(c"SetWindowCompositionAttribute".as_ptr().cast()),
        )?;
        Some(std::mem::transmute::<
            unsafe extern "system" fn() -> isize,
            SetWindowCompositionAttribute,
        >(proc))
    });

fn set_acrylic(h: isize, tint: u32) -> bool {
    let Some(set) = *SET_COMPOSITION else {
        return false;
    };
    let mut accent = AccentPolicy {
        state: ACCENT_ENABLE_ACRYLICBLURBEHIND,
        flags: 2,
        gradient: tint,
        animation: 0,
    };
    let mut data = CompositionData {
        attribute: WCA_ACCENT_POLICY,
        data: &mut accent as *mut _ as *mut c_void,
        size: std::mem::size_of::<AccentPolicy>(),
    };
    unsafe { set(hwnd(h), &mut data) != 0 }
}

/// The accent ignores window regions; DWM's own (fixed radius) corners do clip it.
fn set_corners(h: isize, rounded: bool) {
    let pref = if rounded {
        DWMWCP_ROUND
    } else {
        DWMWCP_DONOTROUND
    };
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd(h),
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const _ as *const c_void,
            std::mem::size_of_val(&pref) as u32,
        );
    }
}

/// On Windows 11 the accent's tint *color* sets the brightness almost regardless of its
/// alpha (AABBGGRR): a mid grey keeps the glass see-through.
fn accent_tint(dark: bool) -> u32 {
    if dark { 0x18_30_30_30 } else { 0x10_98_98_98 }
}

// ---------------- window thread ----------------

unsafe extern "system" fn wnd_proc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_NCHITTEST => LRESULT(-1), // HTTRANSPARENT
        WM_BACKDROP_SHAPE => {
            let width = (wp.0 & 0xffff) as f32;
            let height = ((wp.0 >> 16) & 0xffff) as f32;
            if let Err(err) = scene_shape(width, height, lp.0 as f32) {
                log::debug!("backdrop shape: {err}");
            }
            LRESULT(0)
        }
        WM_BACKDROP_TINT => {
            if let Err(err) = scene_tint(wp.0 as u32) {
                log::debug!("backdrop tint: {err}");
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            SCENE.with_borrow_mut(|s| *s = None);
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(h, msg, wp, lp) },
    }
}

/// Creates the window on its own thread (it needs a message loop).
fn create_window() -> Option<(isize, Mode)> {
    let (tx, rx) = mpsc::channel::<Option<(isize, Mode)>>();
    crate::utils::spawn_named("dock-backdrop", move || run_window(tx));
    rx.recv().ok().flatten()
}

fn run_window(tx: Sender<Option<(isize, Mode)>>) {
    unsafe {
        // Windows.UI.Composition needs a dispatcher queue on this thread
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let _queue = CreateDispatcherQueueController(DispatcherQueueOptions {
            dwSize: std::mem::size_of::<DispatcherQueueOptions>() as u32,
            threadType: DQTYPE_THREAD_CURRENT,
            apartmentType: DQTAT_COM_STA,
        });

        let instance = GetModuleHandleW(None).unwrap_or_default();
        let class = w!("SideDockBackdrop");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassW(&wc); // fails harmlessly when already registered
        let created = CreateWindowExW(
            // not layered: DWM does not draw the acrylic accent on layered windows;
            // no redirection bitmap: only the composition visuals are drawn
            WS_EX_NOREDIRECTIONBITMAP
                | WS_EX_TRANSPARENT
                | WS_EX_TOOLWINDOW
                | WS_EX_NOACTIVATE
                | WS_EX_TOPMOST,
            class,
            w!("SideDock backdrop"),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(instance.into()),
            None,
        );
        let Ok(window) = created else {
            log::warn!("backdrop window not created: {created:?}");
            let _ = tx.send(None);
            return;
        };
        let mode = match (&_queue, build_scene(window)) {
            (Ok(_), Ok(scene)) => {
                SCENE.with_borrow_mut(|s| *s = Some(scene));
                Mode::Composition
            }
            (queue, scene) => {
                log::info!(
                    "composition backdrop unavailable ({:?} / {:?}), using the acrylic accent",
                    queue.as_ref().err(),
                    scene.err()
                );
                Mode::Accent
            }
        };
        let _ = tx.send(Some((window.0 as isize, mode)));
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn post(h: isize, msg: u32, wp: usize, lp: isize) {
    unsafe {
        let _ = PostMessageW(Some(hwnd(h)), msg, WPARAM(wp), LPARAM(lp));
    }
}

fn destroy(b: &Backdrop) {
    post(b.hwnd, WM_CLOSE, 0, 0);
}

fn tint_for(mode: Mode, dark: bool) -> u32 {
    match mode {
        Mode::Composition => composition_tint(dark),
        Mode::Accent => accent_tint(dark),
    }
}

fn apply_tint(b: &Backdrop) {
    match b.mode {
        Mode::Composition => post(b.hwnd, WM_BACKDROP_TINT, b.tint as usize, 0),
        Mode::Accent => {
            set_acrylic(b.hwnd, b.tint);
        }
    }
}

// ---------------- per-frame sync ----------------

/// Keeps one backdrop per dock in place. `enabled` = the frosted glass theme is active.
pub fn sync(docks: &[DockState], enabled: bool, dark: bool) {
    let mut backdrops = BACKDROPS.lock();
    backdrops.retain(|label, b| {
        let keep = enabled && docks.iter().any(|d| &d.label == label);
        if !keep {
            destroy(b);
        }
        keep
    });
    if !enabled {
        return;
    }
    for dock in docks {
        let Some(shape) = dock.backdrop else {
            continue;
        };
        if !backdrops.contains_key(&dock.label) {
            let Some((h, mode)) = create_window() else {
                continue;
            };
            let b = Backdrop {
                hwnd: h,
                mode,
                rect: Rect::default(),
                shape: (0, 0, -1),
                visible_frames: 0,
                shown: false,
                tint: tint_for(mode, dark),
                frames: 0,
            };
            apply_tint(&b);
            backdrops.insert(dock.label.clone(), b);
        }
        let b = backdrops.get_mut(&dock.label).expect("inserted above");
        update(b, dock, shape, dark);
    }
}

fn update(b: &mut Backdrop, dock: &DockState, shape: BackdropShape, dark: bool) {
    let visible = dock.ready
        && dock.shown
        && !dock.hidden
        && !dock.fullscreen_hidden
        && super::native::is_visible(dock.hwnd);

    let t = tint_for(b.mode, dark);
    if t != b.tint {
        b.tint = t;
        apply_tint(b);
    }

    if !visible {
        b.visible_frames = 0;
        if b.shown {
            unsafe {
                let _ = ShowWindow(hwnd(b.hwnd), SW_HIDE);
            }
            b.shown = false;
        }
        return;
    }
    // appear once the page's content has (mostly) slid back in
    b.visible_frames = b.visible_frames.saturating_add(1);
    if !b.shown && b.visible_frames < SHOW_DELAY_FRAMES {
        return;
    }

    let rect = dock.to_physical(shape.x, shape.y, shape.width, shape.height);
    let radius = (shape.radius * dock.scale).round() as i32;
    let next = (rect.width(), rect.height(), radius);
    if next != b.shape {
        match b.mode {
            Mode::Composition => post(
                b.hwnd,
                WM_BACKDROP_SHAPE,
                (next.0.clamp(0, 0xffff) as usize) | ((next.1.clamp(0, 0xffff) as usize) << 16),
                radius as isize,
            ),
            Mode::Accent => {
                if (radius > 0) != (b.shape.2 > 0) || b.shape.2 < 0 {
                    set_corners(b.hwnd, radius > 0);
                }
            }
        }
        b.shape = next;
    }

    // re-slotted below the dock from time to time: the dock re-asserts topmost now and then
    b.frames = b.frames.wrapping_add(1);
    if rect != b.rect || !b.shown || b.frames.is_multiple_of(REORDER_EVERY) {
        let flags = if b.shown {
            SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS
        } else {
            SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS | SWP_SHOWWINDOW
        };
        let _ = unsafe {
            SetWindowPos(
                hwnd(b.hwnd),
                Some(hwnd(dock.hwnd)),
                rect.left,
                rect.top,
                rect.width(),
                rect.height(),
                flags,
            )
        };
        b.shown = true;
        b.rect = rect;
    }
}

/// Destroys every backdrop (exit).
pub fn shutdown() {
    for (_, b) in BACKDROPS.lock().drain() {
        destroy(&b);
    }
}
