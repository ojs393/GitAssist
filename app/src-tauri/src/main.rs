#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod tray;

use tauri::{AppHandle, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};

/// Creates the settings window on demand. Closing it destroys the web view,
/// so the background process holds no browser engine in memory.
pub fn show_main(app: &AppHandle) {
    show_section(app, None);
}

/// Window work happens on the main thread, and on macOS the whole app is
/// brought forward first: a window opened by the background (menu bar) copy
/// otherwise stays behind other apps and its page never draws.
pub fn show_section(app: &AppHandle, section: Option<&str>) {
    let handle = app.clone();
    let section = section.map(str::to_string);
    let _ = app.run_on_main_thread(move || show_now(&handle, section.as_deref()));
}

fn show_now(app: &AppHandle, section: Option<&str>) {
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
        let _ = app.show();
    }
    if let Some(w) = app.get_webview_window("main") {
        if let Some(s) =
            section.filter(|s| s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        {
            let _ = w.eval(format!("location.hash = '{s}'"));
        }
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        return;
    }
    let start = section
        .filter(|s| s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or("");
    let built = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .initialization_script(format!("window.__GITASSIST_SECTION__ = '{start}';"))
        .title("GitAssist")
        .inner_size(1120.0, 780.0)
        .min_inner_size(860.0, 560.0)
        .center()
        .focused(true)
        .visible(true)
        .build();
    if let Ok(w) = built {
        let _ = w.set_focus();
    }
}

fn section_arg(args: &[String]) -> Option<String> {
    args.iter()
        .position(|a| a == "--section")
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn main() {
    // Started from the Dock or at login, the app only sees /usr/bin:/bin; find gh, git and agents like a terminal would.
    if let Ok(env) = gitassist_setup::Env::detect() {
        gitassist_setup::envpath::fix_process_path(&env.home);
    }
    let args: Vec<String> = std::env::args().collect();
    let background = args.iter().any(|a| a == "--background");
    let section = section_arg(&args);

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if !args.iter().any(|a| a == "--background") {
                show_section(app, section_arg(&args).as_deref());
            }
        }))
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::save_config,
            commands::preview,
            commands::ignore_preview,
            commands::detect_github_user,
            commands::turn_on,
            commands::clear_activity,
            commands::storage,
            commands::repos_list,
            commands::repos_discover,
            commands::clear_storage,
            commands::instructions,
            commands::health,
            commands::install_step,
            commands::disable_autostart,
            commands::uninstall_step,
            commands::repair,
            commands::uninstall_all,
            commands::adapter_plan,
            commands::apply_adapter,
            commands::remove_adapter,
            commands::activity,
            commands::scan_repos,
            commands::connect_repo,
            commands::disconnect_repo,
            commands::targets_get,
            commands::targets_save,
            commands::targets_push,
            commands::repo_work,
            commands::repos_work_summaries,
            commands::overrides_get,
            commands::overrides_save,
            commands::overrides_clear_tab,
            commands::jobs_list,
            commands::jobs_cancel,
            commands::jobs_resume,
            commands::job_start_publish,
            commands::job_start_fix_all,
            commands::job_start_apply,
        ])
        .setup(move |app| {
            if let Ok(paths) = gitassist_gateway::Paths::resolve() {
                let _ = gitassist_gateway::jobs::pause_running(&paths);
            }
            let handle = app.handle().clone();
            tray::create(&handle)?;
            tray::spawn_monitor(handle.clone());
            if background {
                #[cfg(target_os = "macos")]
                let _ = handle.set_activation_policy(tauri::ActivationPolicy::Accessory);
            } else {
                show_section(&handle, section.as_deref());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to start GitAssist");

    app.run(|app, event| match event {
        // Closing the last window keeps GitAssist in the tray; only "Quit" exits.
        RunEvent::ExitRequested {
            api, code: None, ..
        } => {
            api.prevent_exit();
            #[cfg(target_os = "macos")]
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let _ = app;
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => show_main(app),
        _ => {}
    });
}
