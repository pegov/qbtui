use std::{
    io,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use tokio::{
    select,
    sync::{mpsc::Receiver, Mutex},
    time::sleep,
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Corner, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Wrap,
    },
    Frame, Terminal,
};

use crate::{
    app::{Action, App, Notification, Route, SortOrder},
    model::TorrentInfo,
};

#[derive(Debug)]
pub enum UiEvent {
    Tick,
    Redraw,
}

pub async fn start_ui(app: Arc<Mutex<App>>, ui_rx: Receiver<UiEvent>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    run(&mut terminal, Arc::clone(&app), ui_rx).await?;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    let app = app.lock().await;
    if let Some(ref reason) = app.forced_shutdown_reason {
        tracing::error!("Forced shutdown!");
        eprintln!("{reason}");
    }

    Ok(())
}

fn create_centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}

fn draw_torrents<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();

    let should_show_search_block =
        app.current_route == Route::Search || !app.search_value.is_empty();

    let constraints = if should_show_search_block {
        [
            Constraint::Percentage(89),
            Constraint::Percentage(6),
            Constraint::Percentage(5),
        ]
        .as_ref()
    } else {
        [Constraint::Percentage(95), Constraint::Percentage(5)].as_ref()
    };

    let rects = Layout::default().constraints(constraints).split(size);

    let torrents_rect = rects[0];
    let stats_rect = if should_show_search_block {
        rects[2]
    } else {
        rects[1]
    };

    let create_block = |title, style| {
        Block::default()
            .borders(Borders::ALL)
            .style(style)
            .title(Span::styled(
                title,
                Style::default().add_modifier(Modifier::BOLD),
            ))
    };

    if should_show_search_block {
        let mut search_value = app.search_value.clone();
        if app.current_route == Route::Search {
            search_value.push('_');
        }

        let search_title = if let Route::Search = app.current_route {
            "Search (Enter - apply, Esc - discard)"
        } else {
            ""
        };

        let text = Paragraph::new(vec![Spans::from(search_value.as_str())])
            .block(create_block(search_title, Style::default()))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        f.render_widget(text, rects[1]);
    }

    let stats_text = app.transfer_info.to_stats_string(&app.host);
    let text = Paragraph::new(vec![Spans::from(stats_text.as_str())])
        .block(create_block("", Style::default()))
        .alignment(Alignment::Right)
        .wrap(Wrap { trim: true });

    f.render_widget(text, stats_rect);

    app.torrents_table_rect = Some(torrents_rect);

    let normal_style = Style::default();
    let selected_style = Style::default().add_modifier(Modifier::REVERSED);

    let category_header = match app.category_sort_order {
        Some(SortOrder::Asc) => "Category ⏷",
        Some(SortOrder::Desc) => "Category ⏶",
        None => "Category",
    };

    let name_header = match app.name_sort_order {
        Some(SortOrder::Asc) => "Name ⏷",
        Some(SortOrder::Desc) => "Name ⏶",
        None => "Name",
    };

    let status_icon_header = match app.status_sort_order {
        Some(SortOrder::Asc) => "⏷",
        Some(SortOrder::Desc) => "⏶",
        None => "",
    };

    let headers = [
        category_header,
        status_icon_header,
        name_header,
        "Size",
        "%",
        "Seeds",
        "Peers",
        "Down",
        "Up",
        "Eta",
    ];
    let cells = headers
        .into_iter()
        .map(|h| Cell::from(h).style(Style::default()));

    let head_row = Row::new(cells)
        .style(normal_style)
        .height(1)
        .bottom_margin(1);

    app.torrents_table.items = app
        .get_visible_torrents()
        .into_iter()
        .map(TorrentInfo::to_row)
        .collect();

    let rows: Vec<Row> = app
        .torrents_table
        .items
        .iter()
        .map(|item| {
            let height = item
                .iter()
                // NOTE: probably breaks mouse
                .map(|content| content.chars().filter(|c| *c == '\n').count())
                .max()
                .unwrap_or(0)
                + 1;
            let cells = item.iter().map(|c| Cell::from(Text::from(c.as_str())));
            Row::new(cells).height(height as u16).bottom_margin(0)
        })
        .collect();

    let table_constraints = [
        Constraint::Percentage(10), // category
        Constraint::Percentage(1),  // status icon
        Constraint::Percentage(35), // name
        Constraint::Percentage(8),  // size
        Constraint::Percentage(5),  // progress
        Constraint::Percentage(5),  // seeds
        Constraint::Percentage(5),  // leechs
        Constraint::Percentage(10), // up
        Constraint::Percentage(10), // dl
        Constraint::Percentage(11), // eta
    ];
    let table = Table::new(rows)
        .header(head_row)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title("Torrents")
                .title_alignment(Alignment::Center),
        )
        .highlight_style(selected_style)
        .highlight_symbol("> ")
        .widths(&table_constraints);

    f.render_stateful_widget(table, rects[0], &mut app.torrents_table.state);
}

fn draw_sort<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();

    let area = create_centered_rect(40, 40, size);
    app.sort_list_rect = Some(area);

    let block = Block::default()
        .title("Toggle sort options")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    let category_option = match app.category_sort_order {
        Some(SortOrder::Asc) => "Category ⏷",
        Some(SortOrder::Desc) => "Category ⏶",
        None => "Category",
    };

    let name_option = match app.name_sort_order {
        Some(SortOrder::Asc) => "Name ⏷",
        Some(SortOrder::Desc) => "Name ⏶",
        None => "Name",
    };

    let status_option = match app.status_sort_order {
        Some(SortOrder::Asc) => "Status ⏷",
        Some(SortOrder::Desc) => "Status ⏶",
        None => "Status",
    };

    let sort_options = vec![
        category_option.to_owned(),
        name_option.to_owned(),
        status_option.to_owned(),
    ];

    app.sort_list.items = sort_options;

    let items: Vec<ListItem> = app
        .sort_list
        .items
        .iter()
        .map(|c| ListItem::new(c.as_str()))
        .collect();

    let list = List::new(items)
        .block(block)
        .start_corner(Corner::TopLeft)
        .style(Style::default())
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");

    if app.sort_list.state.selected().is_none() {
        app.sort_list.state.select(Some(0));
    }

    f.render_widget(Clear, area);
    f.render_stateful_widget(list, area, &mut app.sort_list.state);
}

fn draw_help<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();

    let text = include_str!("../docs/keys.md");
    let block = Block::default()
        .title("Help")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Left)
        .scroll((app.help_state.scroll, 0));

    f.render_widget(paragraph, size);
}

fn draw_categories<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();
    app.categories_list_rect = Some(size);

    let block = Block::default()
        .title("Select category")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    let mut categories = vec!["All".to_string(), "Uncategorized".to_string()];

    categories.extend_from_slice(&app.categories);

    app.categories_list.items = categories.clone();

    let items: Vec<ListItem> = app
        .categories_list
        .items
        .iter()
        .map(|c| ListItem::new(c.as_str()))
        .collect();

    let list = List::new(items)
        .block(block)
        .start_corner(Corner::TopLeft)
        .style(Style::default())
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");

    f.render_stateful_widget(list, size, &mut app.categories_list.state);
}

fn draw_notification<B: Backend>(f: &mut Frame<B>, title: &str, text: &str) {
    let size = f.size();
    let area = create_centered_rect(70, 40, size);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    let text = vec![
        Spans::from(Span::styled(
            title,
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Spans::from(Span::raw("")),
        Spans::from(Span::raw(text)),
    ];

    let paragraph = Paragraph::new(text)
        .style(Style::default())
        .block(block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    f.render_widget(Clear, area);
    f.render_widget(paragraph, area);
}

fn draw_dialog<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();

    let width = std::cmp::min(size.width - 2, 60);
    let height = 14;

    let left = (size.width - width) / 2;
    let top = size.height / 4;

    let rect = Rect::new(left, top, width, height);

    f.render_widget(Clear, rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    f.render_widget(block, rect);

    let vchunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([Constraint::Min(9), Constraint::Length(3)].as_ref())
        .split(rect);

    let torrent_name = app.get_selected_torrent().as_ref().unwrap().name.clone();
    let question = match app.current_action.as_ref().unwrap() {
        Action::Delete => "Are you sure you want to delete the torrent?",
        Action::DeleteFiles => "Are you sure you want to delete the torrent AND FILES?",
    };
    let text = vec![
        Spans::from(Span::raw(question)),
        Spans::from(Span::raw("")),
        Spans::from(Span::styled(
            torrent_name,
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];

    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Center);

    f.render_widget(paragraph, vchunks[0]);

    let hchunks = Layout::default()
        .direction(Direction::Horizontal)
        .horizontal_margin(3)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)].as_ref())
        .split(vchunks[1]);

    let (style1, style2) = match app.confirm {
        true => (
            Style::default().add_modifier(Modifier::REVERSED),
            Style::default(),
        ),
        false => (
            Style::default(),
            Style::default().add_modifier(Modifier::REVERSED),
        ),
    };

    let ok_paragraph = Paragraph::new("Ok")
        .style(style1)
        .alignment(Alignment::Center);

    f.render_widget(ok_paragraph, hchunks[0]);

    let cancel_paragraph = Paragraph::new("Cancel")
        .style(style2)
        .alignment(Alignment::Center);

    f.render_widget(cancel_paragraph, hchunks[1]);
}

fn draw_info<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();

    let block = Block::default()
        .title("Info")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    let torrent = app.get_selected_torrent().unwrap();
    let paragraph = Paragraph::new(torrent.to_info_page())
        .block(block)
        .alignment(Alignment::Left)
        .scroll((app.info_state.scroll, 0));

    f.render_widget(paragraph, size);
}

fn draw_files<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();
    app.files_list_rect = Some(size);

    let block = Block::default()
        .title("Select file")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    app.files_list.items = app
        .current_torrent_files
        .as_ref()
        .unwrap()
        .iter()
        .map(|f| f.name.clone())
        .collect();

    let items: Vec<ListItem> = app
        .files_list
        .items
        .iter()
        .map(|f| ListItem::new(f.as_str()))
        .collect();

    let list = List::new(items)
        .block(block)
        .start_corner(Corner::TopLeft)
        .style(Style::default())
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");

    f.render_stateful_widget(list, size, &mut app.files_list.state);
}

pub async fn run<B: Backend>(
    terminal: &mut Terminal<B>,
    app: Arc<Mutex<App>>,
    mut ui_rx: Receiver<UiEvent>,
) -> Result<()> {
    // first draw
    {
        let mut app = app.lock().await;
        terminal.draw(|f| draw_torrents(f, &mut app))?;
    }

    let mut event_stream = EventStream::new();
    let tick_rate = Duration::from_millis(1000);
    let mut last_tick = Instant::now();
    let mut redraw = true;

    loop {
        if redraw {
            let mut app = app.lock().await;
            let _ = terminal.draw(|f| {
                match app.current_route {
                    Route::Torrents | Route::Search | Route::Dialog => draw_torrents(f, &mut app),
                    Route::Sort => {
                        draw_torrents(f, &mut app);
                        draw_sort(f, &mut app);
                    }
                    Route::Help => draw_help(f, &mut app),
                    Route::Categories => draw_categories(f, &mut app),
                    Route::Info => draw_info(f, &mut app),
                    Route::Files => draw_files(f, &mut app),
                }

                if app.is_connected && app.current_action.is_some() {
                    draw_dialog(f, &mut app);
                }

                if let Some(ref notification) = app.notification {
                    match notification {
                        Notification::FileNotFound => draw_notification(
                            f,
                            "File not found",
                            "File not found or remote server",
                        ),
                    }
                }

                if !app.is_connected {
                    let text = format!(
                        "Connection error! Trying to reconnect... {}",
                        app.error_reconnection_attempt_n
                    );
                    draw_notification(f, "Connection error", &text);
                }
            });
        }

        redraw = false;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        select! {
            maybe_event = event_stream.next() => {
                match maybe_event {
                    Some(Ok(e)) => match e {
                        Event::Key(e) => {
                            let mut app = app.lock().await;
                            if app.is_connected {
                                if app.notification.is_some() {
                                    app.handle_notification_key_event(e).await;
                                } else {
                                    app.handle_key_event(e).await;
                                }
                            } else {
                                app.handle_disconnected_key_event(e).await;
                            }
                            redraw = true;
                        }
                        Event::Mouse(e) => {
                            let mut app = app.lock().await;
                            if app.is_connected && app.notification.is_none() {
                                app.handle_mouse_event(e).await;
                            }
                            redraw = true;
                        }
                        Event::Resize(_, _) => {
                            redraw = true;
                        }
                        _ => {}
                    }
                    Some(Err(err)) => {
                        let mut app = app.lock().await;
                        dbg!(err);
                        app.is_running = false;
                    }
                    None => {
                        let mut app = app.lock().await;
                        app.is_running = false;
                    }
                }
            }
            _ = sleep(timeout) => {
                let mut app = app.lock().await;
                app.sync().await;
                app.trace_send_sync_event_n += 1;
                redraw = true;
            }
            maybe_event = ui_rx.recv() => {
                match maybe_event {
                    Some(e) => match e {
                        UiEvent::Tick => {
                            let app = app.lock().await;
                            app.sync().await;
                            redraw = true;
                        }
                        UiEvent::Redraw => {
                            redraw = true;
                        }
                    }
                    None => {
                        let mut app = app.lock().await;
                        app.is_running = false;
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }

        let app = app.lock().await;
        if !app.is_running {
            break;
        }
    }

    Ok(())
}
