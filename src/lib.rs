pub mod amazon;

#[cfg(feature = "native")]
pub mod adapters;
#[cfg(feature = "native")]
pub mod api;
pub mod application;
#[cfg(feature = "audio-codec")]
pub mod audio_codec;
#[cfg(feature = "native")]
pub mod audio_encoding;
#[cfg(feature = "native")]
pub mod backup;
#[cfg(feature = "native")]
pub mod db;
#[cfg(feature = "native")]
mod db_models;
pub mod domain;
#[cfg(feature = "native")]
mod external;
#[cfg(feature = "native")]
pub mod media_sync;
pub mod ports;

#[cfg(feature = "native")]
use reqwest::Client;
#[cfg(feature = "native")]
use std::sync::Arc;
#[cfg(feature = "native")]
use tokio::sync::Notify;

#[cfg(feature = "native")]
#[derive(Clone)]
pub struct AppState {
    pub db: db::Db,
    pub client: Client,
    pub client_ipv4: Client,
    pub images_dir: Arc<String>,
    pub audio_dir: Arc<String>,
    pub audio_encoding_notify: Arc<Notify>,
    pub epubs_dir: Arc<String>,
    pub uploads_dir: Arc<String>,
    pub object_storage: Arc<adapters::native_object_storage::NativeObjectStorage>,
    pub discogs_token: String,
    pub musicbrainz_contact: String,
}
