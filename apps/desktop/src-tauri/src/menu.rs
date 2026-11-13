// Built in full, rather than from defaults, so every title is in the app's language.
#[cfg(target_os = "macos")]
use objc2::MainThreadMarker;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSApplication, NSImage};
#[cfg(target_os = "macos")]
use objc2_foundation::NSString;
use tauri::menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu};
#[cfg(target_os = "macos")]
use tauri::menu::{HELP_SUBMENU_ID, WINDOW_SUBMENU_ID};
use tauri::{AppHandle, Emitter};

use crate::i18n::text;

const CHECK_FOR_UPDATES: &str = "check-for-updates";
const SETTINGS: &str = "settings";
// Not every system has a predefined quit item, so the others share this one.
#[cfg(not(target_os = "macos"))]
const QUIT: &str = "quit";

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    rebuild(app)?;
    app.on_menu_event(|app, event| match event.id().as_ref() {
        CHECK_FOR_UPDATES => {
            if let Err(error) = app.emit("check-for-updates", ()) {
                eprintln!("Could not ask for an update check: {error}");
            }
        }
        SETTINGS => {
            if let Err(error) = app.emit("toggle-settings", ()) {
                eprintln!("Could not toggle the settings: {error}");
            }
        }
        #[cfg(not(target_os = "macos"))]
        QUIT => app.exit(0),
        _ => {}
    });
    Ok(())
}

// Called again when the language changes.
pub fn rebuild(app: &AppHandle) -> tauri::Result<()> {
    let name = app.package_info().name.clone();
    let named = [("name", name.as_str())];
    let item = |id: &str, key: &str, accelerator: Option<&str>| {
        MenuItem::with_id(app, id, text(key, &named), true, accelerator)
    };
    let separator = || PredefinedMenuItem::separator(app);
    // Otherwise the app is named after its executable when it runs outside the app bundle, as in
    // development.
    let about = PredefinedMenuItem::about(
        app,
        Some(&text("menu.about", &named)),
        Some(AboutMetadata {
            name: Some(name.clone()),
            version: Some(app.package_info().version.to_string()),
            copyright: app.config().bundle.copyright.clone(),
            ..Default::default()
        }),
    )?;
    #[cfg(target_os = "macos")]
    let menu = Menu::with_items(
        app,
        &[
            &Submenu::with_items(
                app,
                &name,
                true,
                &[
                    &about,
                    &item(CHECK_FOR_UPDATES, "menu.checkForUpdates", None)?,
                    &separator()?,
                    &item(SETTINGS, "menu.settings", Some("CmdOrCtrl+,"))?,
                    &separator()?,
                    &PredefinedMenuItem::services(app, Some(&text("menu.services", &[])))?,
                    &separator()?,
                    &PredefinedMenuItem::hide(app, Some(&text("menu.hide", &named)))?,
                    &PredefinedMenuItem::hide_others(app, Some(&text("menu.hideOthers", &[])))?,
                    &PredefinedMenuItem::show_all(app, Some(&text("menu.showAll", &[])))?,
                    &separator()?,
                    &PredefinedMenuItem::quit(app, Some(&text("menu.quit", &named)))?,
                ],
            )?,
            &Submenu::with_items(
                app,
                text("menu.file", &[]),
                true,
                &[&PredefinedMenuItem::close_window(
                    app,
                    Some(&text("menu.closeWindow", &[])),
                )?],
            )?,
            &Submenu::with_items(
                app,
                text("menu.edit", &[]),
                true,
                &[
                    &PredefinedMenuItem::undo(app, Some(&text("menu.undo", &[])))?,
                    &PredefinedMenuItem::redo(app, Some(&text("menu.redo", &[])))?,
                    &separator()?,
                    &PredefinedMenuItem::cut(app, Some(&text("menu.cut", &[])))?,
                    &PredefinedMenuItem::copy(app, Some(&text("menu.copy", &[])))?,
                    &PredefinedMenuItem::paste(app, Some(&text("menu.paste", &[])))?,
                    &PredefinedMenuItem::select_all(app, Some(&text("menu.selectAll", &[])))?,
                ],
            )?,
            &Submenu::with_items(
                app,
                text("menu.view", &[]),
                true,
                &[&PredefinedMenuItem::fullscreen(
                    app,
                    Some(&text("menu.fullscreen", &[])),
                )?],
            )?,
            &Submenu::with_id_and_items(
                app,
                WINDOW_SUBMENU_ID,
                text("menu.window", &[]),
                true,
                &[
                    &PredefinedMenuItem::minimize(app, Some(&text("menu.minimize", &[])))?,
                    &PredefinedMenuItem::maximize(app, Some(&text("menu.zoom", &[])))?,
                    &separator()?,
                    &PredefinedMenuItem::close_window(app, Some(&text("menu.closeWindow", &[])))?,
                ],
            )?,
            &Submenu::with_id_and_items(app, HELP_SUBMENU_ID, text("menu.help", &[]), true, &[])?,
        ],
    )?;
    #[cfg(not(target_os = "macos"))]
    let menu = Menu::with_items(
        app,
        &[
            &Submenu::with_items(
                app,
                text("menu.file", &[]),
                true,
                &[
                    &item(SETTINGS, "menu.settings", Some("CmdOrCtrl+,"))?,
                    &separator()?,
                    &item(QUIT, "menu.quit", None)?,
                ],
            )?,
            &Submenu::with_items(
                app,
                text("menu.edit", &[]),
                true,
                &[
                    &PredefinedMenuItem::cut(app, Some(&text("menu.cut", &[])))?,
                    &PredefinedMenuItem::copy(app, Some(&text("menu.copy", &[])))?,
                    &PredefinedMenuItem::paste(app, Some(&text("menu.paste", &[])))?,
                    &PredefinedMenuItem::select_all(app, Some(&text("menu.selectAll", &[])))?,
                ],
            )?,
            &Submenu::with_items(
                app,
                text("menu.help", &[]),
                true,
                &[
                    &item(CHECK_FOR_UPDATES, "menu.checkForUpdates", None)?,
                    &separator()?,
                    &about,
                ],
            )?,
        ],
    )?;
    app.set_menu(menu)?;
    #[cfg(target_os = "macos")]
    set_symbols(
        app,
        vec![
            (
                text("menu.checkForUpdates", &named),
                "arrow.triangle.2.circlepath",
            ),
            (text("menu.settings", &named), "gear"),
        ],
    );
    Ok(())
}

// System symbols to match the predefined items; the menu library can attach only fixed bitmaps,
// so items are found by title.
#[cfg(target_os = "macos")]
fn set_symbols(app: &AppHandle, symbols: Vec<(String, &'static str)>) {
    let scheduled = app.run_on_main_thread(move || {
        let Some(main_thread) = MainThreadMarker::new() else {
            return;
        };
        let app_menu = NSApplication::sharedApplication(main_thread)
            .mainMenu()
            .and_then(|menu| menu.itemAtIndex(0))
            .and_then(|item| item.submenu());
        let Some(app_menu) = app_menu else {
            return;
        };
        for (title, symbol) in symbols {
            if let Some(item) = app_menu.itemWithTitle(&NSString::from_str(&title)) {
                let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
                    &NSString::from_str(symbol),
                    None,
                );
                item.setImage(image.as_deref());
            }
        }
    });
    if let Err(error) = scheduled {
        eprintln!("Could not set the menu symbols: {error}");
    }
}
