mod agent;

use agent::Agent;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
async fn ask(app: AppHandle, agent: State<'_, Agent>, prompt: String) -> Result<String, String> {
    agent.ask(&app, prompt).await
}

#[tauri::command]
async fn cancel(agent: State<'_, Agent>) -> Result<(), String> {
    agent.cancel().await;
    Ok(())
}

#[tauri::command]
async fn answer(agent: State<'_, Agent>, key: String, yes: bool) -> Result<(), String> {
    agent.answer(&key, yes).await;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Agent::default())
        .setup(|app| {
            // Fullscreen and chromeless; CALM_WINDOWED=1 keeps an ordinary window for development.
            if let Some(window) = app.get_webview_window("main") {
                if std::env::var_os("CALM_WINDOWED").is_some() {
                    let _ = window.set_decorations(true);
                } else {
                    let _ = window.set_fullscreen(true);
                }
                let _ = window.show();
            }
            let app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                app.state::<Agent>().wake(&app).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![ask, cancel, answer])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
