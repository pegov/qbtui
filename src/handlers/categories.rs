use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::{App, PubState, Route};

pub async fn handle_key_event(key_event: KeyEvent, app: &mut App) {
    #[allow(clippy::single_match)]
    match key_event {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            ..
        } => match code {
            KeyCode::Char('q') | KeyCode::Char('c') | KeyCode::Esc => {
                app.current_route = Route::Torrents;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                next_category(app);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                prev_category(app);
            }
            KeyCode::Enter => {
                app.choose_selected_category();
                app.current_route = Route::Torrents;
            }
            _ => {}
        },
        _ => {}
    }
}

pub async fn handle_mouse_event(mouse_event: MouseEvent, app: &mut App) {
    if let MouseEventKind::Down(MouseButton::Left) = mouse_event.kind {
        app.left_click = (mouse_event.column, mouse_event.row);

        if let Some(rect) = app.categories_list_rect {
            // HARDCODE
            let rect_row_start = rect.y + 1;
            let rect_col_start = rect.x + 1;
            let rect_col_end = rect.x + rect.width;
            if app.left_click.0 >= rect_col_start
                && app.left_click.0 <= rect_col_end
                && app.left_click.1 >= rect_row_start
            {
                let mut i: usize = (app.left_click.1 - rect_row_start).into();

                // SAFETY: UNSAFE
                unsafe {
                    let state: &PubState = std::mem::transmute(&app.categories_list.state);
                    i += state.offset;
                }

                if app.categories_list.items.len() > i {
                    app.categories_list.state.select(Some(i));
                    app.choose_selected_category();
                    app.current_route = Route::Torrents;
                }
            }
        }
    }
}

fn next_category(app: &mut App) {
    let i = match app.categories_list.state.selected() {
        Some(i) => {
            if i >= app.categories_list.items.len() - 1 {
                0
            } else {
                i + 1
            }
        }
        None => 0,
    };
    app.categories_list.state.select(Some(i));
}

fn prev_category(app: &mut App) {
    let i = match app.categories_list.state.selected() {
        Some(i) => {
            if i == 0 {
                app.categories_list.items.len() - 1
            } else {
                i - 1
            }
        }
        None => 0,
    };
    app.categories_list.state.select(Some(i));
}
