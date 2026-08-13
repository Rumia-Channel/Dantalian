use serde::{Deserialize, Serialize};

use super::author::Author;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BookRecord {
    pub id: i64,
    pub isbn: Option<String>,
    pub isdn: Option<String>,
    pub jan: Option<String>,
    pub title: String,
    pub publisher: Option<String>,
    pub publish_date: Option<String>,
    pub cover_url: Option<String>,
    pub description: Option<String>,
    pub title_transcription: Option<String>,
    pub series_title: Option<String>,
    pub series_title_transcription: Option<String>,
    pub alternative: Option<String>,
    pub alternative_transcription: Option<String>,
    pub volume: Option<String>,
    pub volume_transcription: Option<String>,
    pub price: Option<String>,
    pub extent: Option<String>,
    pub jpno: Option<String>,
    pub ndl_url: Option<String>,
    pub series_id: Option<i64>,
    pub series_number: Option<i64>,
    pub isdn_region: Option<String>,
    pub isdn_class: Option<String>,
    pub isdn_type: Option<String>,
    pub isdn_rating_gender: Option<String>,
    pub isdn_rating_age: Option<String>,
    pub isdn_genre_code: Option<String>,
    pub isdn_genre_name: Option<String>,
    pub isdn_genre_user: Option<String>,
    pub isdn_c_code: Option<String>,
    pub isdn_author: Option<String>,
    pub isdn_shape: Option<String>,
    pub isdn_contents: Option<String>,
    pub isdn_barcode2: Option<String>,
    pub isdn_sample_image_url: Option<String>,
    pub isdn_useroption: Option<String>,
    pub isdn_external_links: Option<String>,
    pub media_type: Option<String>,
    pub catalog_number: Option<String>,
    pub artist: Option<String>,
    pub label: Option<String>,
    pub disc_count: Option<i64>,
    pub epub_file_hash: Option<String>,
    pub epub_file_name: Option<String>,
    pub reading_status: Option<String>,
    pub storage_location_id: Option<i64>,
    pub label_id: Option<i64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BookAuthor {
    pub id: i64,
    pub ndl_id: Option<String>,
    pub name: String,
    pub transcription: Option<String>,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BookWithAuthors {
    #[serde(flatten)]
    pub book: BookRecord,
    pub authors: Vec<BookAuthor>,
    #[serde(default)]
    pub copies_count: i64,
    #[serde(default)]
    pub lent_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Track {
    pub id: i64,
    pub book_id: Option<i64>,
    pub cd_id: Option<i64>,
    pub disc_number: i64,
    pub track_number: i64,
    pub title: String,
    pub duration: Option<String>,
    pub file_hash: Option<String>,
    pub file_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Cd {
    pub id: i64,
    pub jan: Option<String>,
    pub title: String,
    pub artist: Option<String>,
    pub publisher: Option<String>,
    pub label: Option<String>,
    pub catalog_number: Option<String>,
    pub publish_date: Option<String>,
    pub cover_url: Option<String>,
    pub description: Option<String>,
    pub disc_count: Option<i64>,
    pub volume: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub parent_book_id: Option<i64>,
    pub media_type: Option<String>,
    pub series_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CdWithTracks {
    #[serde(flatten)]
    pub cd: Cd,
    pub track_artist: Option<String>,
    pub album_artist: Option<String>,
    pub tracks: Vec<Track>,
    pub authors: Vec<BookAuthor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CopyRecord {
    pub id: i64,
    pub book_id: i64,
    pub copy_type: String,
    pub location: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CopyWithStatus {
    #[serde(flatten)]
    pub copy: CopyRecord,
    pub lent_to: Option<String>,
    pub lent_date: Option<String>,
    pub due_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LendingRecord {
    pub id: i64,
    pub copy_id: i64,
    pub borrower_id: i64,
    pub borrower_name: Option<String>,
    pub lent_date: String,
    pub due_date: Option<String>,
    pub returned_date: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GrandSeriesItem {
    pub item_type: String,
    pub item_id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GrandSeriesWithItems {
    pub id: i64,
    pub name: String,
    pub items: Vec<GrandSeriesItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub cover_cd_id: Option<i64>,
    pub cover_url: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaylistTrackEntry {
    pub position: i64,
    pub track: Track,
    pub cd: Cd,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaylistWithTracks {
    #[serde(flatten)]
    pub playlist: Playlist,
    pub tracks: Vec<PlaylistTrackEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct TrackMetadata {
    pub track_id: i64,
    pub title: Option<String>,
    pub track_number: Option<i64>,
    pub track_total: Option<i64>,
    pub disc_number: Option<i64>,
    pub disc_total: Option<i64>,
    pub comment: Option<String>,
    pub encoder: Option<String>,
    pub lyrics: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub artist: Option<String>,
    pub publisher: Option<String>,
    pub label: Option<String>,
    pub year: Option<i64>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub isrc: Option<String>,
    pub file_type: Option<String>,
    pub raw_size_bytes: Option<i64>,
    pub replay_gain_track_gain_db: Option<f64>,
    pub replay_gain_track_peak: Option<f64>,
    pub replay_gain_album_gain_db: Option<f64>,
    pub replay_gain_album_peak: Option<f64>,
    pub artists: Vec<Author>,
    pub album_artists: Vec<Author>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct CdMetadata {
    pub cd_id: i64,
    pub year: Option<i64>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub isrc: Option<String>,
    pub replay_gain_album_gain_db: Option<f64>,
    pub replay_gain_album_peak: Option<f64>,
}
