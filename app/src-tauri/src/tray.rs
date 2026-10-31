use std::time::Duration;

use gitassist_core::Config;
use gitassist_gateway::Paths;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

const RECHECK: Duration = Duration::from_secs(15 * 60);

pub struct TrayItems {
    status: MenuItem<Wry>,
    pause: MenuItem<Wry>,
}

fn enabled() -> bool {
    Paths::resolve()
        .ok()
        .and_then(|p| Config::load(&p.config_file).ok())
        .is_some_and(|c| c.enabled)
}

fn toggle_enabled() {
    let Ok(p) = Paths::resolve() else { return };
    let Ok(mut c) = Config::load(&p.config_file) else {
        return;
    };
    c.enabled = !c.enabled;
    let _ = c.save_atomic(&p.config_file);
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let status = MenuItem::with_id(app, "status", "Checking…", false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open GitAssist", true, None::<&str>)?;
    let check = MenuItem::with_id(app, "check", "Run health check", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", pause_label(), true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit GitAssist", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&status, &sep, &open, &check, &pause, &sep, &quit])?;

    // macOS tints a black template image for light/dark menu bars; other
    // platforms need the coloured icon to stay visible on dark taskbars.
    let icon = if cfg!(target_os = "macos") {
        tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?
    } else {
        app.default_window_icon()
            .cloned()
            .ok_or(tauri::Error::InvalidIcon(std::io::Error::other(
                "no app icon",
            )))?
    };
    TrayIconBuilder::with_id("main")
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("GitAssist")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => crate::show_main(app),
            "check" => refresh(app),
            "pause" => {
                toggle_enabled();
                if let Some(items) = app.try_state::<TrayItems>() {
                    let _ = items.pause.set_text(pause_label());
                }
                refresh(app);
            }
            "quit" => {
                if let Ok(paths) = Paths::resolve() {
                    let _ = gitassist_gateway::jobs::pause_running(&paths);
                }
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;
    app.manage(TrayItems { status, pause });
    Ok(())
}

fn pause_label() -> &'static str {
    if enabled() {
        "Pause enforcement"
    } else {
        "Resume enforcement"
    }
}

/// Runs the health check off the UI thread and updates the tray.
pub fn refresh(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let text = match (Paths::resolve(), gitassist_setup::Env::detect()) {
            (Ok(p), Ok(e)) => {
                if let Ok(cli) = crate::commands::bundled_cli() {
                    gitassist_setup::ops::heal(&p, &e, &cli, crate::commands::app_exe().as_deref());
                }
                let r = gitassist_setup::health::check_with(&p, &e, false);
                if !enabled() {
                    "Paused".to_string()
                } else if r.ok {
                    "Protected".to_string()
                } else {
                    "Needs attention: open GitAssist".to_string()
                }
            }
            _ => "Settings could not be read".to_string(),
        };
        if let Some(items) = app.try_state::<TrayItems>() {
            let _ = items.status.set_text(&text);
        }
        if let Some(tray) = app.tray_by_id("main") {
            let _ = tray.set_tooltip(Some(format!("GitAssist: {text}")));
        }
    });
}

/// One sleeping thread: heal and re-check every 15 minutes. No polling loop, no CPU while idle.
pub fn spawn_monitor(app: AppHandle) {
    std::thread::spawn(move || loop {
        refresh(&app);
        std::thread::sleep(RECHECK);
    });
}
