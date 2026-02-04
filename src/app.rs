use std::time::SystemTime;

use crossterm::event::{KeyEvent, MouseEvent};
use tokio::sync::mpsc::Sender;
use tui::{
    layout::Rect,
    widgets::{ListState, TableState},
};

use crate::{
    api::ApiEvent,
    handlers,
    model::{TorrentFile, TorrentInfo, TransferInfo},
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Route {
    #[default]
    Torrents,
    Sort,
    Categories,
    Search,
    Help,
    Info,
    Files,
    Dialog,
}

#[derive(Debug, Default)]
pub struct TorrentsTable {
    pub state: TableState,
    pub items: Vec<Vec<String>>,
}

#[derive(Debug, Default)]
pub struct AppListState {
    pub state: ListState,
    pub items: Vec<String>,
}

#[derive(Debug, Default)]
pub struct ScrollableTextState {
    pub scroll: u16,
    pub text_height: usize,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub enum SelectedCategory {
    #[default]
    All,
    Uncategorized,
    Category(usize),
}

#[derive(Debug)]
pub enum Action {
    Delete,
    DeleteFiles,
}

#[derive(Debug)]
pub enum Notification {
    FileNotFound,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SortOrder {
    Asc,
    Desc,
}

pub fn next_sort_order(curr: &Option<SortOrder>) -> Option<SortOrder> {
    match curr {
        Some(SortOrder::Asc) => Some(SortOrder::Desc),
        Some(SortOrder::Desc) => None,
        None => Some(SortOrder::Asc),
    }
}

#[derive(Clone, Debug)]
pub struct PathRewrite {
    pub from: String,
    pub to: String,
}

#[derive(Debug)]
pub struct App {
    pub host: String,
    pub api_tx: Sender<ApiEvent>,

    pub is_connected: bool,
    pub is_running: bool,
    pub forced_shutdown_reason: Option<String>,

    pub error_reconnection_attempt_n: usize,

    pub notification: Option<Notification>,

    pub torrents: Vec<TorrentInfo>,
    pub current_torrent: Option<TorrentInfo>, // for files and info
    pub transfer_info: TransferInfo,
    pub categories: Vec<String>,

    pub current_route: Route,
    pub on_help_route: Option<Route>,
    pub on_error_route: Option<Route>,

    pub selected_category: SelectedCategory,

    pub torrents_table: TorrentsTable,
    pub torrents_table_rect: Option<Rect>,

    pub categories_list: AppListState,
    pub categories_list_rect: Option<Rect>,

    pub info_state: ScrollableTextState,

    pub current_torrent_files: Option<Vec<TorrentFile>>,
    pub files_list: AppListState,
    pub files_list_rect: Option<Rect>,

    pub search_value: String,

    pub help_state: ScrollableTextState,

    pub current_action: Option<Action>,
    pub confirm: bool,

    pub category_sort_order: Option<SortOrder>,
    pub name_sort_order: Option<SortOrder>,
    pub status_sort_order: Option<SortOrder>,

    pub sort_list: AppListState,
    pub sort_list_rect: Option<Rect>,

    pub left_click: (u16, u16),
    pub left_click_ts: SystemTime,

    pub trace_send_sync_event_n: usize,
    pub trace_handle_sync_event_n: usize,

    pub remote: bool,
    pub path_rewrites: Option<Vec<PathRewrite>>,
}

impl App {
    pub fn new(
        host: &str,
        api_tx: Sender<ApiEvent>,
        remote: bool,
        path_rewrites: Option<Vec<PathRewrite>>,
    ) -> Self {
        let mut categories_list = AppListState::default();
        categories_list.state.select(Some(0)); // select "All" by default

        Self {
            host: host.to_owned(),
            api_tx,

            is_connected: true,
            is_running: true,
            forced_shutdown_reason: None,

            error_reconnection_attempt_n: 0,

            notification: None,

            torrents: vec![],
            current_torrent: None,
            transfer_info: TransferInfo::default(),
            categories: vec![],

            current_route: Route::Torrents,
            on_help_route: None,
            on_error_route: None,

            selected_category: SelectedCategory::default(),

            torrents_table: TorrentsTable::default(),
            torrents_table_rect: None,

            categories_list,
            categories_list_rect: None,

            info_state: ScrollableTextState::default(),

            current_torrent_files: None,
            files_list: AppListState::default(),
            files_list_rect: None,

            search_value: String::new(),

            help_state: ScrollableTextState::default(),

            current_action: None,
            confirm: false,

            category_sort_order: Some(SortOrder::Asc),
            name_sort_order: Some(SortOrder::Asc),
            status_sort_order: Some(SortOrder::Asc),

            sort_list: AppListState::default(),
            sort_list_rect: None,

            left_click: (0, 0),
            left_click_ts: SystemTime::now(),

            trace_send_sync_event_n: 0,
            trace_handle_sync_event_n: 0,

            remote,
            path_rewrites,
        }
    }

    pub fn rewrite_path(&self, path: &str) -> String {
        if let Some(ref rewrites) = self.path_rewrites {
            for rewrite in rewrites {
                if path.starts_with(&rewrite.to) {
                    return path.replacen(&rewrite.to, &rewrite.from, 1);
                }
            }
        }
        path.to_string()
    }

    pub async fn handle_key_event(&mut self, event: KeyEvent) {
        tracing::debug!("key_event: {:?}", &event);
        match self.current_route {
            Route::Torrents => {
                handlers::torrents::handle_key_event(event, self).await;
            }
            Route::Sort => {
                handlers::sort::handle_key_event(event, self).await;
            }
            Route::Search => {
                handlers::search::handle_key_event(event, self).await;
            }
            Route::Help => {
                handlers::help::handle_key_event(event, self).await;
            }
            Route::Categories => {
                handlers::categories::handle_key_event(event, self).await;
            }
            Route::Dialog => {
                handlers::dialog::handle_key_event(event, self).await;
            }
            Route::Info => {
                handlers::info::handle_key_event(event, self).await;
            }
            Route::Files => {
                handlers::files::handle_key_event(event, self).await;
            }
        }
    }

    pub async fn handle_notification_key_event(&mut self, event: KeyEvent) {
        tracing::debug!("notification_key_event: {:?}", &event);
        handlers::notification::handle_key_event(event, self).await;
    }

    pub async fn handle_disconnected_key_event(&mut self, event: KeyEvent) {
        tracing::debug!("disconnected_key_event: {:?}", &event);
        handlers::error::handle_key_event(event, self).await;
    }

    pub async fn handle_mouse_event(&mut self, event: MouseEvent) {
        tracing::debug!("mouse_event: {:?}", &event);
        match self.current_route {
            Route::Torrents => {
                handlers::torrents::handle_mouse_event(event, self).await;
            }
            Route::Sort => {
                handlers::sort::handle_mouse_event(event, self).await;
            }
            Route::Categories => {
                handlers::categories::handle_mouse_event(event, self).await;
            }
            _ => {}
        }
    }

    pub fn get_visible_torrents(&self) -> Vec<&TorrentInfo> {
        // filter by category
        let torrents: Vec<&TorrentInfo> = match self.selected_category {
            SelectedCategory::All => self.torrents.iter().collect(),
            SelectedCategory::Uncategorized => self
                .torrents
                .iter()
                .filter(|t| t.category.is_empty())
                .collect(),
            SelectedCategory::Category(i) => {
                let category = &self.categories[i - 2];
                self.torrents
                    .iter()
                    .filter(|t| &t.category == category)
                    .collect()
            }
        };

        // filter by name
        let normal_value = self.search_value.trim().to_lowercase();
        let dotted_value = normal_value.split(' ').collect::<Vec<&str>>().join(".");

        let mut res: Vec<&TorrentInfo> = torrents
            .into_iter()
            .filter(|item| {
                let torrent_name = item.name.to_lowercase();
                torrent_name.contains(&normal_value) || torrent_name.contains(&dotted_value)
            })
            .collect();

        // sort
        if let Some(sort_order) = &self.name_sort_order {
            match sort_order {
                SortOrder::Asc => res.sort_by(|a, b| a.name.partial_cmp(&b.name).unwrap()),
                SortOrder::Desc => res.sort_by(|a, b| b.name.partial_cmp(&a.name).unwrap()),
            }
        }

        if let Some(sort_order) = &self.category_sort_order {
            match sort_order {
                SortOrder::Asc => {
                    res.sort_by(|a, b| a.category.partial_cmp(&b.category).unwrap());
                }
                SortOrder::Desc => {
                    if self.selected_category == SelectedCategory::All {
                        res.sort_by(|a, b| b.category.partial_cmp(&a.category).unwrap());
                    }
                }
            }
        }

        if let Some(sort_order) = &self.status_sort_order {
            match sort_order {
                SortOrder::Asc => res.sort_by(|a, b| (a.state as i32).cmp(&(b.state as i32))),
                SortOrder::Desc => res.sort_by(|a, b| (b.state as i32).cmp(&(a.state as i32))),
            }
        }

        res
    }

    pub fn get_selected_torrent(&self) -> Option<&TorrentInfo> {
        self.torrents_table
            .state
            .selected()
            .and_then(|i| self.get_visible_torrents().get(i).copied())
    }

    pub fn select_first_torrent(&mut self) {
        if self.get_visible_torrents().is_empty() {
            return;
        }

        self.torrents_table.state.select(Some(0));
    }

    pub async fn sync(&self) {
        self.api_tx.send(ApiEvent::Sync).await.unwrap()
    }

    pub fn choose_selected_category(&mut self) {
        if let Some(i) = self.categories_list.state.selected() {
            self.selected_category = match i {
                0 => SelectedCategory::All,
                1 => SelectedCategory::Uncategorized,
                i => SelectedCategory::Category(i),
            };
            self.torrents_table.state.select(None);
        }
    }

    pub fn set_current_action(&mut self, action: Action) {
        self.current_action = Some(action);
        self.confirm = false;
        self.current_route = Route::Dialog;
    }

    pub fn reset_current_action(&mut self) {
        self.current_action = None;
        self.confirm = false;
        self.current_route = Route::Torrents;
    }

    pub async fn apply_current_action(&mut self) {
        if self.confirm {
            if let Some(torrent) = self.get_selected_torrent() {
                if let Some(ref action) = self.current_action {
                    match action {
                        Action::Delete => {
                            self.api_tx
                                .send(ApiEvent::Delete(torrent.hash.clone()))
                                .await
                                .unwrap();
                        }
                        Action::DeleteFiles => {
                            self.api_tx
                                .send(ApiEvent::DeleteFiles(torrent.hash.clone()))
                                .await
                                .unwrap();
                        }
                    }
                }
            }
        }

        self.reset_current_action();
        self.current_route = Route::Torrents;
    }
}
