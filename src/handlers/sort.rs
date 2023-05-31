use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::{next_sort_order, App, Route};

pub async fn handle_key_event(key_event: KeyEvent, app: &mut App) {
    #[allow(clippy::single_match)]
    match key_event {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            ..
        } => match code {
            KeyCode::Char('q') | KeyCode::Char('t') | KeyCode::Esc => {
                app.current_route = Route::Torrents;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                next_sort_target(app);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                prev_sort_target(app);
            }
            KeyCode::Enter => {
                if let Some(i) = app.sort_list.state.selected() {
                    handle_sort_order_change(app, i);
                }
            }
            _ => {}
        },
        _ => {}
    }
}

pub async fn handle_mouse_event(mouse_event: MouseEvent, app: &mut App) {
    if let MouseEventKind::Down(MouseButton::Left) = mouse_event.kind {
        app.left_click = (mouse_event.column, mouse_event.row);

        if let Some(rect) = app.sort_list_rect {
            // HARDCODE
            let rect_row_start = rect.y + 1;
            let rect_col_start = rect.x + 1;
            let rect_col_end = rect.x + rect.width;
            if app.left_click.0 >= rect_col_start
                && app.left_click.0 <= rect_col_end
                && app.left_click.1 >= rect_row_start
            {
                let mut i: usize = (app.left_click.1 - rect_row_start).into();
                i += app.categories_list.state.offset();

                if app.sort_list.items.len() > i {
                    app.sort_list.state.select(Some(i));
                    handle_sort_order_change(app, i);
                }
            }
        }
    }
}

fn next_sort_target(app: &mut App) {
    let i = match app.sort_list.state.selected() {
        Some(i) => {
            if i >= app.sort_list.items.len() - 1 {
                0
            } else {
                i + 1
            }
        }
        None => 0,
    };
    app.sort_list.state.select(Some(i));
}

fn prev_sort_target(app: &mut App) {
    let i = match app.sort_list.state.selected() {
        Some(i) => {
            if i == 0 {
                app.sort_list.items.len() - 1
            } else {
                i - 1
            }
        }
        None => 0,
    };
    app.sort_list.state.select(Some(i));
}

fn handle_sort_order_change(app: &mut App, i: usize) {
    match i {
        0 => app.category_sort_order = next_sort_order(&app.category_sort_order),
        1 => app.name_sort_order = next_sort_order(&app.name_sort_order),
        2 => app.status_sort_order = next_sort_order(&app.status_sort_order),
        _ => unreachable!(),
    }
}
