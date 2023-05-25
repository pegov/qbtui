use std::{collections::HashMap, path::Path, sync::Arc, time::Duration};

use reqwest::{Client, Response};
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    sync::{mpsc::Sender, Mutex},
    try_join,
};

use crate::{
    app::{App, Notification, Route, SelectedCategory},
    model::{
        Category, DeleteTorrentParams, GetMainDataParams, GetTorrentFilesParams,
        GetTorrentListParams, Hashes, LoginPayload, MainData, SpeedLimitsMode, TorrentFile,
        TorrentInfo, TransferInfo,
    },
    ui::UiEvent,
};

#[derive(Clone, Debug)]
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
pub struct Api {
    client: Client,
    base_url: String,
    username: Option<String>,
    password: Option<String>,
}

#[derive(Debug)]
pub enum LoginError {
    WrongCredentials,
    TooManyAttempts,
}

#[derive(Debug)]
pub enum ExternalError {
    Connection(reqwest::Error),
    Internal,
}

#[derive(Debug)]
pub enum ApiError {
    External(ExternalError),
    NotAuthenticated,
    Login(LoginError),
}

impl From<LoginError> for ApiError {
    fn from(value: LoginError) -> Self {
        Self::Login(value)
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(value: reqwest::Error) -> Self {
        Self::External(ExternalError::Connection(value))
    }
}

impl Api {
    fn new(
        base_url: &str,
        do_not_verify_webui_certificate: bool,
        username: Option<String>,
        password: Option<String>,
    ) -> Self {
        let client = reqwest::ClientBuilder::new()
            .cookie_store(true)
            .danger_accept_invalid_certs(do_not_verify_webui_certificate)
            .build()
            .expect("Could not build reqwest client");

        Self {
            client,
            base_url: base_url.to_owned(),
            username,
            password,
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
        let res = self
            .client
            .get(self.build_url(path))
            .query(&query)
            .send()
            .await?;
        if res.status() == 403 {
            return Err(ApiError::NotAuthenticated);
        }

        Ok(res.text().await?)
    }

    async fn get_json<T, Q>(&self, path: &str, query: Option<Q>) -> Result<T, ApiError>
    where
        T: for<'de> DeserializeOwned,
        Q: Serialize,
    {
        let res = self
            .client
            .get(self.build_url(path))
            .query(&query)
            .send()
            .await?;
        if res.status() == 403 {
            return Err(ApiError::NotAuthenticated);
        }

        Ok(res.json().await?)
    }

    async fn post<P: Serialize>(
        &self,
        path: &str,
        payload: Option<P>,
    ) -> Result<Response, ApiError> {
        let res = self
            .client
            .post(self.build_url(path))
            .form(&payload)
            .send()
            .await?;
        if res.status() == 403 {
            return Err(ApiError::NotAuthenticated);
        }

        Ok(res)
    }

    async fn post_with_timeout<P: Serialize>(
        &self,
        path: &str,
        payload: Option<P>,
        timeout: Duration,
    ) -> Result<Response, ApiError> {
        let res = self
            .client
            .post(self.build_url(path))
            .form(&payload)
            .timeout(timeout)
            .send()
            .await?;
        if res.status() == 403 {
            return Err(ApiError::NotAuthenticated);
        }

        Ok(res)
    }

    pub async fn login(&mut self) -> Result<(), ApiError> {
        // 200, Ok. - ok
        // 200, Fails. - wrong creds
        // 403 - too many attempts
        let payload = LoginPayload::new(
            self.username.as_ref().unwrap(),
            self.password.as_ref().unwrap(),
        );
        let res = self.post("/auth/login", Some(payload)).await?;

        if res.status() == 200 {
            let body = res.text().await.unwrap();
            match body.as_str() {
                "Ok." => Ok(()),
                "Fails." => Err(LoginError::WrongCredentials.into()),
                _ => unreachable!(),
            }
        } else if res.status() == 403 {
            return Err(LoginError::TooManyAttempts.into());
        } else {
            return Err(ApiError::External(ExternalError::Internal));
        }
    }

    pub async fn logout(&mut self) -> Result<(), ApiError> {
        tracing::debug!("Logout");
        self.post_with_timeout::<()>("/auth/logout", None, Duration::from_millis(500))
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
        self.post("/torrents/pause", Some(payload)).await?;
        Ok(())
    }

    async fn resume(&self, hashes: &[&str]) -> Result<(), ApiError> {
        let payload = Hashes::from(hashes);
        self.post("/torrents/resume", Some(payload)).await?;
        Ok(())
    }

    async fn categories(&self) -> Result<HashMap<String, Category>, ApiError> {
        self.get_json::<_, ()>("/torrents/categories", None).await
    }

    async fn delete(&self, payload: DeleteTorrentParams) -> Result<(), ApiError> {
        self.post("/torrents/delete", Some(payload)).await?;
        Ok(())
    }

    async fn sync_maindata(&self, query: GetMainDataParams) -> Result<MainData, ApiError> {
        self.get_json("/sync/maindata", Some(query)).await
    }
}

pub struct ApiHandler {
    app: Arc<Mutex<App>>,
    ui_tx: Sender<UiEvent>,
    pub api: Api,
    rid: i64,
    current_event: ApiEvent,
}

impl ApiHandler {
    pub fn new(
        app: Arc<Mutex<App>>,
        ui_tx: Sender<UiEvent>,
        base_url: &str,
        do_not_verify_webui_certificate: bool,
        username: Option<String>,
        password: Option<String>,
    ) -> Self {
        Self {
            api: Api::new(
                base_url,
                do_not_verify_webui_certificate,
                username,
                password,
            ),
            ui_tx,
            app,
            rid: 0,
            current_event: ApiEvent::Sync,
        }
    }

    pub async fn handle(&mut self, event: ApiEvent) -> Result<(), ApiError> {
        tracing::debug!(?event);
        self.current_event = event.clone();
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
            ApiError::External(inner) => {
                tracing::warn!(?inner);
                let mut app = self.app.lock().await;
                app.is_connected = false;
                app.error_reconnection_attempt_n += 1;
                app.current_route = Route::Torrents;
            }
            ApiError::NotAuthenticated => {
                {
                    let app = self.app.lock().await;
                    if !app.is_running {
                        return;
                    }
                }
                tracing::warn!("Handling new session...");
                if self.api.login().await.is_err() {
                    let mut app = self.app.lock().await;
                    app.is_running = false;
                    app.forced_shutdown_reason = Some("Could not relogin".to_owned());
                    tracing::warn!("New session was not handled!");
                    return;
                }
                if self.handle(self.current_event.clone()).await.is_err() {
                    let mut app = self.app.lock().await;
                    app.is_running = false;
                    app.forced_shutdown_reason = Some("Not authenticated".to_owned());
                    tracing::warn!("New session was not handled!");
                    return;
                };
                tracing::warn!("New session was successfully handled!");
            }
            ApiError::Login(_) => unreachable!(),
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
