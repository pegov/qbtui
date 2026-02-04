use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, Notification, Route};

pub async fn handle_key_event(key_event: KeyEvent, app: &mut App) {
    if let KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        ..
    } = key_event
    {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                app.current_torrent_files = None;
                app.current_route = Route::Torrents;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                next_file(app);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                prev_file(app);
            }
            KeyCode::Char('o') | KeyCode::Enter => {
                if !app.remote {
                    open_file(app);
                }
            }
            _ => {}
        }
    }
}

fn next_file(app: &mut App) {
    let i = match app.files_list.state.selected() {
        Some(i) => {
            if i >= app.files_list.items.len() - 1 {
                0
            } else {
                i + 1
            }
        }
        None => 0,
    };
    app.files_list.state.select(Some(i));
}

fn prev_file(app: &mut App) {
    let i = match app.files_list.state.selected() {
        Some(i) => {
            if i == 0 {
                app.files_list.items.len() - 1
            } else {
                i - 1
            }
        }
        None => 0,
    };
    app.files_list.state.select(Some(i));
}

fn open_file(app: &mut App) {
    if let Some(i) = app.files_list.state.selected() {
        let file = &app.current_torrent_files.as_ref().unwrap()[i];
        let content_path = &app.current_torrent.as_ref().unwrap().content_path;
        let rewritten_content_path = app.rewrite_path(content_path);
        let path = Path::new(&rewritten_content_path).parent().unwrap();
        let path = path.join(&file.name);
        if path.exists() {
            open::that_in_background(path);
        } else {
            app.notification = Some(Notification::FileNotFound);
        }
    }
}
