use std::{collections::HashMap, path::Path, sync::Arc};

use reqwest::Client;
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    sync::{mpsc::Sender, Mutex},
    try_join,
};

use crate::{
    app::{App, Notification, Route, SelectedCategory},
    model::{
        Category, DeleteTorrentParams, GetMainDataParams, GetTorrentFilesParams,
        GetTorrentListParams, Hashes, MainData, SpeedLimitsMode, TorrentFile, TorrentInfo,
        TransferInfo,
    },
    ui::UiEvent,
};

#[derive(Debug)]
pub enum ApiEvent {
    Reload,
    Sync,
    Files(String),
    Delete(String),
    DeleteFiles(String),
    Pause(String),
    Resume(String),
}

#[derive(Debug)]
struct Api {
    client: Client,
    base_url: String,
}

#[derive(Debug)]
pub enum ApiError {
    Connection(reqwest::Error),
}

impl From<reqwest::Error> for ApiError {
    fn from(value: reqwest::Error) -> Self {
        Self::Connection(value)
    }
}

impl Api {
    fn new(base_url: &str, do_not_verify_webui_certificate: bool) -> Self {
        let client = reqwest::ClientBuilder::new()
            .cookie_store(true)
            .danger_accept_invalid_certs(do_not_verify_webui_certificate)
            .build()
            .expect("Could not build reqwest client");

        Self {
            client,
            base_url: base_url.to_owned(),
        }
    }

    fn build_url(&self, path: &str) -> String {
        format!("{}{}{}", self.base_url, "/api/v2", path)
    }

    async fn get_text<Q: Serialize>(
        &self,
        path: &str,
        query: Option<Q>,
    ) -> Result<String, ApiError> {
        Ok(self
            .client
            .get(self.build_url(path))
            .query(&query)
            .send()
            .await?
            .text()
            .await?)
    }

    async fn get_json<T, Q>(&self, path: &str, query: Option<Q>) -> Result<T, ApiError>
    where
        T: for<'de> DeserializeOwned,
        Q: Serialize,
    {
        Ok(self
            .client
            .get(self.build_url(path))
            .query(&query)
            .send()
            .await?
            .json()
            .await?)
    }

    async fn post<P: Serialize>(&self, path: &str, payload: Option<P>) -> Result<(), ApiError> {
        self.client
            .post(self.build_url(path))
            .form(&payload)
            .send()
            .await?;

        Ok(())
    }

    async fn transfer_info(&self) -> Result<TransferInfo, ApiError> {
        self.get_json::<_, ()>("/transfer/info", None).await
    }

    async fn transfer_speed_limits_mode(&self) -> Result<SpeedLimitsMode, ApiError> {
        let text = self
            .get_text::<()>("/transfer/speedLimitsMode", None)
            .await?;
        Ok(SpeedLimitsMode::from(text))
    }

    async fn torrents_info(
        &self,
        query: Option<GetTorrentListParams>,
    ) -> Result<Vec<TorrentInfo>, ApiError> {
        self.get_json("/torrents/info", query).await
    }

    async fn torrents_files(
        &self,
        query: GetTorrentFilesParams,
    ) -> Result<Vec<TorrentFile>, ApiError> {
        self.get_json("/torrents/files", Some(query)).await
    }

    async fn pause(&self, hashes: &[&str]) -> Result<(), ApiError> {
        let payload = Hashes::from(hashes);
        self.post("/torrents/pause", Some(payload)).await
    }

    async fn resume(&self, hashes: &[&str]) -> Result<(), ApiError> {
        let payload = Hashes::from(hashes);
        self.post("/torrents/resume", Some(payload)).await
    }

    async fn categories(&self) -> Result<HashMap<String, Category>, ApiError> {
        self.get_json::<_, ()>("/torrents/categories", None).await
    }

    async fn delete(&self, payload: DeleteTorrentParams) -> Result<(), ApiError> {
        self.post("/torrents/delete", Some(payload)).await
    }

    async fn sync_maindata(&self, query: GetMainDataParams) -> Result<MainData, ApiError> {
        self.get_json("/sync/maindata", Some(query)).await
    }
}

pub struct ApiHandler {
    app: Arc<Mutex<App>>,
    ui_tx: Sender<UiEvent>,
    api: Api,
    rid: i64,
}

impl ApiHandler {
    pub fn new(
        app: Arc<Mutex<App>>,
        ui_tx: Sender<UiEvent>,
        base_url: &str,
        do_not_verify_webui_certificate: bool,
    ) -> Self {
        Self {
            api: Api::new(base_url, do_not_verify_webui_certificate),
            ui_tx,
            app,
            rid: 0,
        }
    }

    pub async fn handle(&mut self, event: ApiEvent) -> Result<(), ApiError> {
        tracing::debug!(?event);
        let input_event: Option<UiEvent> = match event {
            ApiEvent::Reload => {
                self.reload().await?;
                None
            }
            ApiEvent::Sync => {
                self.sync().await?;
                let mut app = self.app.lock().await;
                app.trace_handle_sync_event_n += 1;
                None
            }
            ApiEvent::Pause(hash) => {
                self.api.pause(&[&hash]).await?;
                Some(UiEvent::Tick)
            }
            ApiEvent::Resume(hash) => {
                self.api.resume(&[&hash]).await?;
                Some(UiEvent::Tick)
            }
            ApiEvent::Files(hash) => {
                let files = self.api.torrents_files(hash.clone().into()).await?;

                let mut app = self.app.lock().await;
                if let Some(ref torrent) = app.current_torrent {
                    if files.len() == 1 {
                        let path = Path::new(&torrent.content_path);
                        if path.exists() {
                            open::that_in_background(path);
                        } else {
                            app.notification = Some(Notification::FileNotFound);
                        }
                        None
                    } else {
                        app.current_torrent_files = Some(files);
                        app.files_list.state.select(Some(0));
                        app.current_route = Route::Files;
                        Some(UiEvent::Redraw)
                    }
                } else {
                    None
                }
            }
            ApiEvent::Delete(hash) => {
                self.api
                    .delete(DeleteTorrentParams {
                        hashes: hash,
                        delete_files: false,
                    })
                    .await?;
                Some(UiEvent::Tick)
            }
            ApiEvent::DeleteFiles(hash) => {
                self.api
                    .delete(DeleteTorrentParams {
                        hashes: hash,
                        delete_files: true,
                    })
                    .await?;
                Some(UiEvent::Tick)
            }
        };
        {
            let mut app = self.app.lock().await;
            app.is_connected = true;
            app.error_reconnection_attempt_n = 0;
        }
        if let Some(input_event) = input_event {
            self.ui_tx.send(input_event).await.unwrap();
        }
        Ok(())
    }

    pub async fn handle_error(&mut self, e: ApiError) {
        match e {
            ApiError::Connection(inner) => {
                tracing::error!(?inner);
                let mut app = self.app.lock().await;
                app.is_connected = false;
                app.error_reconnection_attempt_n += 1;
                app.current_route = Route::Torrents;
            }
        }
    }

    pub async fn reload(&self) -> Result<(), ApiError> {
        match try_join!(
            self.api.transfer_info(),
            self.api.torrents_info(None),
            self.api.categories(),
            self.api.transfer_speed_limits_mode(),
        ) {
            Ok((transfer_info, torrents_info, categories, transfer_speed_limits_mode)) => {
                let mut app = self.app.lock().await;
                app.torrents = torrents_info;
                app.transfer_info = transfer_info;
                app.transfer_info.use_alt_speed_limits =
                    transfer_speed_limits_mode == SpeedLimitsMode::Alternative;
                let mut categories: Vec<String> = categories.into_keys().collect();
                categories.sort_by_key(|a| a.to_lowercase());
                app.categories = categories;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub async fn sync(&mut self) -> Result<(), ApiError> {
        let data = self
            .api
            .sync_maindata(GetMainDataParams { rid: self.rid })
            .await?;

        self.rid = data.rid;

        if let Some(full_update) = data.full_update {
            if full_update {
                self.reload().await?;
                return Ok(());
            }
        }

        if let Some(torrents_removed) = data.torrents_removed {
            let mut app = self.app.lock().await;
            app.torrents
                .retain(|torrent| !torrents_removed.contains(&torrent.hash));
        }

        let mut should_reload: bool = false;
        if let Some(torrents) = data.torrents {
            let mut app = self.app.lock().await;
            for (hash, info) in torrents {
                if let Some(torrent) = app.torrents.iter_mut().find(|item| item.hash == hash) {
                    macro_rules! replace_if_some {
                        ($name:ident) => {
                            if let Some(v) = info.$name {
                                torrent.$name = v;
                            }
                        };
                    }
                    // NOTE: is it okay???
                    replace_if_some!(added_on);
                    replace_if_some!(amount_left);
                    replace_if_some!(category);
                    replace_if_some!(completed);
                    replace_if_some!(completion_on);
                    replace_if_some!(content_path);
                    replace_if_some!(downloaded);
                    replace_if_some!(eta);
                    replace_if_some!(name);
                    replace_if_some!(progress);
                    replace_if_some!(save_path);
                    replace_if_some!(state);
                    replace_if_some!(size);
                    replace_if_some!(dlspeed);
                    replace_if_some!(upspeed);
                } else {
                    // new torrent?
                    should_reload = true;
                    break;
                }
            }
        }

        if let Some(categories) = data.categories {
            let mut app = self.app.lock().await;

            // NOTE: or just reload if it doen't work
            let new_categories: Vec<String> = categories.into_keys().collect();
            app.categories.extend_from_slice(&new_categories);
            app.categories.sort_unstable();
        }

        if let Some(categories_removed) = data.categories_removed {
            let mut app = self.app.lock().await;

            let selected_category_name = match app.selected_category {
                SelectedCategory::Category(i) => Some(&app.categories[i - 2]),
                _ => None,
            };
            if let Some(selected_category_name) = selected_category_name {
                if categories_removed.contains(selected_category_name) {
                    app.selected_category = SelectedCategory::All;
                }
            }

            app.categories.retain(|c| !categories_removed.contains(c));
            app.categories.sort_unstable();
        }

        if should_reload {
            self.reload().await?;
            return Ok(());
        }

        if let Some(server_state) = data.server_state {
            let mut app = self.app.lock().await;
            macro_rules! replace_if_some {
                ($name:ident) => {
                    if let Some(v) = server_state.$name {
                        app.transfer_info.$name = v;
                    }
                };
            }
            replace_if_some!(dl_info_speed);
            replace_if_some!(dl_info_data);
            replace_if_some!(up_info_speed);
            replace_if_some!(up_info_data);
            replace_if_some!(dl_rate_limit);
            replace_if_some!(up_rate_limit);
            replace_if_some!(dht_nodes);
            replace_if_some!(connection_status);
            replace_if_some!(use_alt_speed_limits);
        }

        Ok(())
    }
}
