use std::{path::Path, time::SystemTime};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::{
    api::ApiEvent,
    app::{Action, App, Notification, PubState, Route},
};

pub async fn handle_key_event(key_event: KeyEvent, app: &mut App) {
    match key_event {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            ..
        } => match code {
            KeyCode::F(1) | KeyCode::Char('?') => {
                app.on_help_route = Some(app.current_route.clone());
                app.current_route = Route::Help;
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                app.is_running = false;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                next_torrent(app);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                prev_torrent(app);
            }
            KeyCode::Char('/') => {
                app.current_route = Route::Search;
            }
            KeyCode::Char('c') => {
                app.current_route = Route::Categories;
            }
            KeyCode::Char('i') => {
                if let Some(torrent) = app.get_selected_torrent() {
                    app.current_torrent = Some(torrent.clone());
                    app.current_route = Route::Info;
                }
            }
            KeyCode::Char('o') | KeyCode::Enter => {
                if app.get_selected_torrent().is_some() {
                    let selected_torrent = app.get_selected_torrent().unwrap().clone();
                    app.current_torrent = Some(app.get_selected_torrent().unwrap().clone());
                    let path = Path::new(&selected_torrent.content_path);
                    if path.exists() {
                        if path.is_file() {
                            open::that_in_background(path);
                        } else {
                            app.api_tx
                                .send(ApiEvent::Files(selected_torrent.hash.clone()))
                                .await
                                .unwrap();
                        }
                    } else {
                        app.notification = Some(Notification::FileNotFound);
                    }
                }
            }
            KeyCode::Char('r') => app.api_tx.send(ApiEvent::Reload).await.unwrap(),
            KeyCode::Char(' ') => {
                if let Some(torrent) = app.get_selected_torrent() {
                    if torrent.is_running() {
                        app.api_tx
                            .send(ApiEvent::Pause(torrent.hash.clone()))
                            .await
                            .unwrap()
                    } else {
                        app.api_tx
                            .send(ApiEvent::Resume(torrent.hash.clone()))
                            .await
                            .unwrap()
                    }
                }
            }
            KeyCode::Char('p') => {
                if let Some(torrent) = app.get_selected_torrent() {
                    app.api_tx
                        .send(ApiEvent::Pause(torrent.hash.clone()))
                        .await
                        .unwrap()
                }
            }
            KeyCode::Char('s') => {
                if let Some(torrent) = app.get_selected_torrent() {
                    app.api_tx
                        .send(ApiEvent::Resume(torrent.hash.clone()))
                        .await
                        .unwrap()
                }
            }
            KeyCode::Char('x') => {
                if app.get_selected_torrent().is_some() {
                    app.set_current_action(Action::Delete);
                }
            }
            KeyCode::Char('t') => {
                app.current_route = Route::Sort;
            }
            _ => {}
        },
        KeyEvent {
            code,
            modifiers: KeyModifiers::SHIFT,
            ..
        } => match code {
            KeyCode::Char('O') => {
                open_folder_in_default_file_manager(app);
            }
            KeyCode::Char('X') => {
                if app.get_selected_torrent().is_some() {
                    app.set_current_action(Action::DeleteFiles);
                }
            }
            _ => {}
        },
        _ => {}
    }
}

pub async fn handle_mouse_event(mouse_event: MouseEvent, app: &mut App) {
    if let MouseEventKind::Down(MouseButton::Left) = mouse_event.kind {
        let elapsed_ms = app.left_click_ts.elapsed().unwrap().as_millis();

        app.left_click = (mouse_event.column, mouse_event.row);
        app.left_click_ts = SystemTime::now();

        if let Some(rect) = app.torrents_table_rect {
            // HARDCODE
            let rect_row_start = rect.y + 3;
            let rect_col_start = rect.x + 1;
            let rect_col_end = rect.x + rect.width;
            if app.left_click.0 >= rect_col_start
                && app.left_click.0 <= rect_col_end
                && app.left_click.1 >= rect_row_start
            {
                let mut i: usize = (app.left_click.1 - rect_row_start).into();

                // SAFETY: UNSAFE
                unsafe {
                    let state: &PubState = std::mem::transmute(&app.torrents_table.state);
                    if state.offset > 0 {
                        i += state.offset;
                    }
                }

                if app.torrents_table.items.len() > i {
                    app.torrents_table.state.select(Some(i));
                }

                // double click
                if elapsed_ms <= 500
                    && app.get_selected_torrent().is_some()
                    && app.current_torrent.is_some()
                {
                    let selected_torrent = app.get_selected_torrent().unwrap();
                    let current_torrent = app.current_torrent.as_ref().unwrap();
                    if selected_torrent.hash == current_torrent.hash {
                        let path = Path::new(&selected_torrent.content_path);
                        if path.exists() {
                            if path.is_file() {
                                open::that_in_background(path);
                            } else {
                                app.api_tx
                                    .send(ApiEvent::Files(selected_torrent.hash.clone()))
                                    .await
                                    .unwrap();
                            }
                        } else {
                            app.notification = Some(Notification::FileNotFound);
                        }
                    }
                }

                if app.get_selected_torrent().is_some() {
                    app.current_torrent = Some(app.get_selected_torrent().unwrap().clone());
                }
            }
        }
    }
}

fn next_torrent(app: &mut App) {
    let i = match app.torrents_table.state.selected() {
        Some(i) => {
            if i >= app.torrents_table.items.len() - 1 {
                0
            } else {
                i + 1
            }
        }
        None => 0,
    };
    app.torrents_table.state.select(Some(i));
}

fn prev_torrent(app: &mut App) {
    let i = match app.torrents_table.state.selected() {
        Some(i) => {
            if i == 0 {
                app.torrents_table.items.len() - 1
            } else {
                i - 1
            }
        }
        None => 0,
    };
    app.torrents_table.state.select(Some(i));
}

fn open_folder_in_default_file_manager(app: &mut App) {
    if let Some(torrent) = app.get_selected_torrent() {
        let path = Path::new(&torrent.content_path);
        if path.is_dir() && path.exists() {
            open::that_in_background(path);
        } else if path.parent().unwrap().exists() {
            open::that_in_background(path.parent().unwrap());
        } else {
            app.notification = Some(Notification::FileNotFound);
        }
    }
}
