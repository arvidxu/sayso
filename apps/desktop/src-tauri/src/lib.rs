mod i18n;
mod menu;
#[cfg(target_os = "macos")]
mod system_language;
mod updates;

use serde::Deserialize;

const SETTINGS: &str = "settings.json";

fn product<T: serde::de::DeserializeOwned>(section: &str) -> T {
    let file: serde_json::Value =
        serde_json::from_str(include_str!("../../../../product.conf.json"))
            .expect("product.conf.json is not valid JSON");
    serde_json::from_value(file[section].clone()).unwrap_or_else(|error| {
        panic!("product.conf.json has no valid \"{section}\" section: {error}")
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppConfig {
    show_window_after_seconds: u64,
    initial_settings: InitialSettings,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitialSettings {
    automatic_updates: bool,
}

fn app_config() -> AppConfig {
    product("app")
}

#[tauri::command]
async fn check_for_updates(app: tauri::AppHandle) -> updates::Outcome {
    updates::check_now(&app).await
}

#[tauri::command]
async fn download_update(app: tauri::AppHandle) -> updates::Outcome {
    updates::download(&app).await
}

#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> updates::Outcome {
    // Unpacking the update takes a while; the window stays responsive meanwhile.
    tauri::async_runtime::spawn_blocking(move || updates::install(&app))
        .await
        .unwrap_or_else(|error| updates::Outcome::Failed {
            message: error.to_string(),
        })
}

#[tauri::command]
fn cancel_update_download(app: tauri::AppHandle) {
    updates::cancel_download(&app);
}

#[tauri::command]
fn take_pending_update(app: tauri::AppHandle) -> Option<updates::Outcome> {
    updates::take_pending(&app)
}

#[tauri::command]
fn skip_update(app: tauri::AppHandle) {
    updates::skip(&app);
}

#[tauri::command]
fn postpone_update(app: tauri::AppHandle) {
    updates::postpone(&app);
}

// A language the user picked, rather than the system's, also becomes the app's preferred
// language in the system.
#[tauri::command]
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
fn set_language(app: tauri::AppHandle, language: String, follows_system: bool) {
    #[cfg(target_os = "macos")]
    system_language::set_preferred((!follows_system).then_some(language.as_str()));
    if i18n::set(&language)
        && let Err(error) = menu::rebuild(&app)
    {
        eprintln!("Could not rebuild the menu: {error}");
    }
}

// Resets what the page cannot; the page resets settings.json itself.
#[tauri::command]
fn restore_defaults(app: tauri::AppHandle, window: tauri::WebviewWindow) -> tauri::Result<()> {
    updates::forget_skipped(&app);
    let Some(config) = app
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == window.label())
    else {
        return Ok(());
    };
    window.set_fullscreen(false)?;
    window.unmaximize()?;
    window.set_size(tauri::LogicalSize::new(config.width, config.height))?;
    window.center()
}

// `None` where only the page can tell.
#[tauri::command]
fn uses_12_hour_clock() -> Option<bool> {
    #[cfg(target_os = "macos")]
    return Some(system_language::uses_12_hour_clock());
    #[cfg(not(target_os = "macos"))]
    None
}

#[tauri::command]
fn get_system_language() -> &'static str {
    i18n::system()
}

// A page that fails to load would leave the app running with no window, so show it anyway
// after a while.
fn show_windows_later(app: tauri::AppHandle) {
    let wait = std::time::Duration::from_secs(app_config().show_window_after_seconds);
    std::thread::spawn(move || {
        std::thread::sleep(wait);
        for window in tauri::Manager::webview_windows(&app).values() {
            if !window.is_visible().unwrap_or(true)
                && let Err(error) = window.show()
            {
                eprintln!("Could not show the window: {error}");
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Visibility is not restored: the page shows the window once its theme is applied.
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::all()
                        - tauri_plugin_window_state::StateFlags::VISIBLE,
                )
                .build(),
        )
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            i18n::setup(app.handle());
            updates::setup(app.handle())?;
            menu::setup(app.handle())?;
            show_windows_later(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            check_for_updates,
            download_update,
            install_update,
            cancel_update_download,
            take_pending_update,
            skip_update,
            postpone_update,
            restore_defaults,
            set_language,
            get_system_language,
            uses_12_hour_clock
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    // The sections are otherwise read only once the app runs, some only in release builds.
    #[test]
    fn the_product_sections_load() {
        super::app_config();
        super::updates::timing();
    }
}
