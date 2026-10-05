mod agent;
mod sessions;

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

#[tauri::command]
fn open_session(id: String) -> Result<(), String> {
    sessions::open(&id)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Agent::default())
        .setup(|app| {
            // An ordinary window; the green title-bar button takes it fullscreen.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus(); // so typing works without a click
            }
            let app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                app.state::<Agent>().wake(&app).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![ask, cancel, answer, open_session])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
