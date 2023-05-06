use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;

pub async fn handle_key_event(key_event: KeyEvent, app: &mut App) {
    #[allow(clippy::single_match)]
    match key_event {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            ..
        } => match code {
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => {
                app.notification = None;
            }
            _ => {}
        },
        _ => {}
    }
}
