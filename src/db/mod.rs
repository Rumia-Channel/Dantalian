pub mod authors;
pub mod books;
pub mod borrowers;
pub mod cd_metadata;
pub mod cds;
pub mod copies;
pub mod schema;
pub mod series;
pub mod settings;
pub mod storage_locations;
pub mod track_authors;
pub mod track_metadata;
pub mod track_metadata_search;
pub mod tracks;

pub use crate::db_models::*;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Db(pub Arc<Mutex<Connection>>);
