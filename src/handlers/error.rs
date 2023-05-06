use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{api::ApiEvent, app::App};

pub async fn handle_key_event(key_event: KeyEvent, app: &mut App) {
    #[allow(clippy::single_match)]
    match key_event {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            ..
        } => match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                app.is_running = false;
            }
            KeyCode::Char('r') => app.api_tx.send(ApiEvent::Reload).await.unwrap(),
            _ => {}
        },
        _ => {}
    }
}
