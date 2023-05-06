use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;

pub async fn handle_key_event(key_event: KeyEvent, app: &mut App) {
    match key_event {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            ..
        } => match code {
            KeyCode::F(1) | KeyCode::Char('q') | KeyCode::Esc => {
                app.current_route = app.on_help_route.take().unwrap();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                app.help_state.scroll += 1;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if app.help_state.scroll >= 1 {
                    app.help_state.scroll -= 1;
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
                app.help_state.scroll += 10;
            }
            KeyCode::Char('K') => {
                if app.help_state.scroll >= 10 {
                    app.help_state.scroll -= 10;
                } else {
                    app.help_state.scroll = 0;
                }
            }
            _ => {}
        },
        _ => {}
    }
}
