//! Tray icon on the native taskbar (which SideDock keeps alive).

use tauri::{
    AppHandle,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::{
    error::{Result, ResultLogExt},
    state::settings::{self, Language},
    widgets,
};

const TRAY_ID: &str = "sidedock-tray";

fn labels(lang: Language) -> [&'static str; 4] {
    match lang {
        Language::ZhCn => [
            "打开 SideDock",
            "重新加载停靠栏",
            "停靠栏：启用/停用",
            "退出",
        ],
        Language::En => [
            "Open SideDock",
            "Reload dock",
            "Dock: enable/disable",
            "Quit",
        ],
    }
}

fn build_menu(app: &AppHandle) -> Result<Menu<tauri::Wry>> {
    let [open, reload, toggle, quit] = labels(settings::get().language);
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", open, true, None::<&str>)?,
            &MenuItem::with_id(app, "reload", reload, true, None::<&str>)?,
            &MenuItem::with_id(app, "toggle", toggle, true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", quit, true, None::<&str>)?,
        ],
    )?;
    Ok(menu)
}

pub fn create(app: &AppHandle) -> Result<()> {
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("missing default icon")?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("SideDock")
        .menu(&build_menu(app)?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => widgets::settings::open(app).log_error(),
            "reload" => crate::app::reload_docks(app),
            "toggle" => {
                let mut s = settings::get();
                s.dock.enabled = !s.dock.enabled;
                crate::app::apply_settings(app, s).log_error();
            }
            "quit" => crate::app::quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                widgets::settings::open(tray.app_handle()).log_error();
            }
        })
        .build(app)?;
    Ok(())
}

/// Rebuilds the menu (e.g. after a language change).
pub fn refresh(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID)
        && let Ok(menu) = build_menu(app)
    {
        tray.set_menu(Some(menu)).log_error();
    }
}
