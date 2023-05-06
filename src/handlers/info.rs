use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, Route};

pub async fn handle_key_event(key_event: KeyEvent, app: &mut App) {
    match key_event {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            ..
        } => match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                app.current_route = Route::Torrents;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                app.info_state.scroll += 1;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if app.info_state.scroll >= 1 {
                    app.info_state.scroll -= 1;
                }
            }
            _ => {}
        },
        KeyEvent {
            code,
            modifiers: KeyModifiers::SHIFT,
            ..
        } => match code {
            KeyCode::Char('J') => {
                app.info_state.scroll += 10;
            }
            KeyCode::Char('K') => {
                if app.info_state.scroll >= 10 {
                    app.info_state.scroll -= 10;
                } else {
                    app.info_state.scroll = 0;
                }
            }
            _ => {}
        },
        _ => {}
    }
}
