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
            KeyCode::Char('q') | KeyCode::Esc => {
                app.reset_current_action();
            }
            KeyCode::Char('h') | KeyCode::Char('l') | KeyCode::Left | KeyCode::Right => {
                app.confirm = !app.confirm;
            }
            KeyCode::Enter => {
                app.apply_current_action().await;
            }
            _ => {}
        },
        _ => {}
    }
}
