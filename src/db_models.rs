use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Author {
    pub id: i64,
    pub ndl_id: Option<String>,
    pub name: String,
    pub transcription: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewAuthor {
    pub ndl_id: Option<String>,
    pub name: String,
    pub transcription: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Book {
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BookAuthor {
    pub id: i64,
    pub ndl_id: Option<String>,
    pub name: String,
    pub transcription: Option<String>,
    pub sort_order: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BookWithAuthors {
    #[serde(flatten)]
    pub book: Book,
    pub authors: Vec<BookAuthor>,
    #[serde(default)]
    pub copies_count: i64,
    #[serde(default)]
    pub lent_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct NewBook {
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
    pub authors: Vec<NewAuthor>,
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
    pub tracks: Option<Vec<NewTrack>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Series {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StorageLocation {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Label {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GrandSeries {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GrandSeriesItemInfo {
    pub item_type: String,
    pub item_id: i64,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GrandSeriesWithItems {
    pub id: i64,
    pub name: String,
    pub items: Vec<GrandSeriesItemInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Copy {
    pub id: i64,
    pub book_id: i64,
    pub copy_type: String,
    pub location: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Borrower {
    pub id: i64,
    pub name: String,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Deserialize)]
pub struct NewLendingRecord {
    pub borrower_id: i64,
    pub due_date: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CopyWithStatus {
    #[serde(flatten)]
    pub copy: Copy,
    pub lent_to: Option<String>,
    pub lent_date: Option<String>,
    pub due_date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Track {
    pub id: i64,
    pub book_id: i64,
    pub cd_id: Option<i64>,
    pub disc_number: i64,
    pub track_number: i64,
    pub title: String,
    pub duration: Option<String>,
    pub file_hash: Option<String>,
    pub file_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewTrack {
    pub disc_number: Option<i64>,
    pub track_number: i64,
    pub title: String,
    pub duration: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CdInfo {
    pub title: String,
    pub artist: Option<String>,
    pub publisher: Option<String>,
    pub label: Option<String>,
    pub catalog_number: Option<String>,
    pub publish_date: Option<String>,
    pub cover_url: Option<String>,
    pub disc_count: Option<i64>,
    pub tracks: Vec<NewTrack>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Deserialize)]
pub struct NewCd {
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
    pub tracks: Option<Vec<NewTrack>>,
    pub parent_book_id: Option<i64>,
    pub media_type: Option<String>,
    pub series_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct CdWithTracks {
    #[serde(flatten)]
    pub cd: Cd,
    pub tracks: Vec<Track>,
    pub authors: Vec<BookAuthor>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CdMetadata {
    pub cd_id: i64,
    pub year: Option<i64>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub isrc: Option<String>,

    #[serde(skip)]
    pub cover_mime: Option<String>,
    #[serde(skip)]
    pub cover_data: Option<Vec<u8>>,

    pub replay_gain_album_gain_db: Option<f64>,
    pub replay_gain_album_peak: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TrackMetadataResponse {
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

    #[serde(skip)]
    pub cover_mime: Option<String>,
    #[serde(skip)]
    pub cover_data: Option<Vec<u8>>,

    pub artists: Vec<BookAuthor>,
    pub album_artists: Vec<BookAuthor>,
}

/// CD 配下の全トラックの track_metadata から得たアルバムレベルタグの合意値。
/// cd 側の編集値(cds/cd_metadata/cd_authors)とは別に出し、UI で「タグ由来(参考)」
/// 表示や空欄への仮入力に使う。各フィールドは disc/track 順で最初の非空値を採用。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AlbumTagConsensus {
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub artist: Option<String>,
    pub publisher: Option<String>,
    pub label: Option<String>,
    pub year: Option<i64>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub lyrics: Option<String>,
}
