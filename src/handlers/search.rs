use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, Route};

pub async fn handle_key_event(key_event: KeyEvent, app: &mut App) {
    match key_event {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            ..
        } => match code {
            KeyCode::Esc => {
                app.search_value = String::from("");
                app.current_route = Route::Torrents;
                app.select_first_torrent();
            }
            KeyCode::Enter => {
                app.current_route = Route::Torrents;
                app.select_first_torrent();
            }
            KeyCode::Backspace => {
                if !app.search_value.is_empty() {
                    app.search_value.pop();
                }
            }
            KeyCode::Char(c) => {
                app.search_value.push(c);
            }
            _ => {}
        },
        KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::SHIFT,
            ..
        } => {
            app.search_value.push(c);
        }
        _ => {}
    }
}
