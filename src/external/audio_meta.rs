use lofty::file::FileType;
use lofty::prelude::TaggedFileExt;
use lofty::tag::{Accessor, ItemKey, Tag, TagType};
use serde::{Deserialize, Serialize};

use crate::db_models::CdMetadata;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TrackMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_number: Option<i64>,
    pub track_total: Option<i64>,
    pub disc_number: Option<i64>,
    pub disc_total: Option<i64>,
    pub year: Option<i64>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub publisher: Option<String>,
    pub label: Option<String>,
    pub encoder: Option<String>,
    pub comment: Option<String>,
    pub lyrics: Option<String>,

    #[serde(skip)]
    pub cover_mime: Option<String>,
    #[serde(skip)]
    pub cover_data: Option<Vec<u8>>,

    pub replay_gain_track_gain_db: Option<f64>,
    pub replay_gain_track_peak: Option<f64>,
    pub replay_gain_album_gain_db: Option<f64>,
    pub replay_gain_album_peak: Option<f64>,

    pub file_type: Option<String>,
    pub raw_size_bytes: Option<i64>,
}

impl TrackMetadata {
    pub fn into_cd_metadata(self, cd_id: i64) -> CdMetadata {
        CdMetadata {
            cd_id,
            year: self.year,
            genre: self.genre,
            composer: self.composer,
            isrc: None,
            cover_mime: self.cover_mime,
            cover_data: self.cover_data,
            replay_gain_album_gain_db: self.replay_gain_album_gain_db,
            replay_gain_album_peak: self.replay_gain_album_peak,
        }
    }
}

impl CdMetadata {
    pub fn from_json(cd_id: i64, body: &serde_json::Value) -> Self {
        fn opt_str(v: &serde_json::Value, k: &str) -> Option<String> {
            v.get(k).and_then(|x| x.as_str()).map(String::from)
        }
        fn opt_i64(v: &serde_json::Value, k: &str) -> Option<i64> {
            v.get(k).and_then(|x| x.as_i64())
        }
        fn opt_f64(v: &serde_json::Value, k: &str) -> Option<f64> {
            v.get(k).and_then(|x| x.as_f64())
        }
        Self {
            cd_id,
            year: opt_i64(body, "year"),
            genre: opt_str(body, "genre"),
            composer: opt_str(body, "composer"),
            isrc: opt_str(body, "isrc"),
            cover_mime: opt_str(body, "cover_mime"),
            cover_data: body
                .get("cover_data")
                .and_then(|x| x.as_array())
                .and_then(|arr| {
                    arr.iter()
                        .map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()))
                        .collect::<Option<Vec<u8>>>()
                }),
            replay_gain_album_gain_db: opt_f64(body, "replay_gain_album_gain_db"),
            replay_gain_album_peak: opt_f64(body, "replay_gain_album_peak"),
        }
    }
}

impl TrackMetadata {
    pub fn from_json(body: &serde_json::Value) -> Self {
        fn opt_str(v: &serde_json::Value, k: &str) -> Option<String> {
            v.get(k).and_then(|x| x.as_str()).map(String::from)
        }
        fn opt_i64(v: &serde_json::Value, k: &str) -> Option<i64> {
            v.get(k).and_then(|x| x.as_i64())
        }
        fn opt_f64(v: &serde_json::Value, k: &str) -> Option<f64> {
            v.get(k).and_then(|x| x.as_f64())
        }
        Self {
            title: opt_str(body, "title"),
            artist: opt_str(body, "artist"),
            album: opt_str(body, "album"),
            album_artist: opt_str(body, "album_artist"),
            track_number: opt_i64(body, "track_number"),
            track_total: opt_i64(body, "track_total"),
            disc_number: opt_i64(body, "disc_number"),
            disc_total: opt_i64(body, "disc_total"),
            year: opt_i64(body, "year"),
            genre: opt_str(body, "genre"),
            composer: opt_str(body, "composer"),
            publisher: opt_str(body, "publisher"),
            label: opt_str(body, "label"),
            encoder: opt_str(body, "encoder"),
            comment: opt_str(body, "comment"),
            lyrics: opt_str(body, "lyrics"),
            cover_mime: opt_str(body, "cover_mime"),
            cover_data: body
                .get("cover_data")
                .and_then(|x| x.as_array())
                .and_then(|arr| {
                    arr.iter()
                        .map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()))
                        .collect::<Option<Vec<u8>>>()
                }),
            replay_gain_track_gain_db: opt_f64(body, "replay_gain_track_gain_db"),
            replay_gain_track_peak: opt_f64(body, "replay_gain_track_peak"),
            replay_gain_album_gain_db: opt_f64(body, "replay_gain_album_gain_db"),
            replay_gain_album_peak: opt_f64(body, "replay_gain_album_peak"),
            file_type: opt_str(body, "file_type"),
            raw_size_bytes: opt_i64(body, "raw_size_bytes"),
        }
    }
}

pub fn extract(path: &std::path::Path) -> Result<TrackMetadata, lofty::error::LoftyError> {
    let raw_size_bytes = std::fs::metadata(path).ok().map(|m| m.len() as i64);

    let tagged = lofty::read_from_path(path)?;
    let file_type = Some(format!("{:?}", tagged.file_type()));

    let mut meta = match tagged.file_type() {
        FileType::Mpeg | FileType::Mp4 | FileType::Wav | FileType::Aiff => {
            extract_universal(tagged.primary_tag())
        }
        FileType::Flac | FileType::Vorbis | FileType::Speex => {
            extract_vorbis_like(tagged.primary_tag())
        }
        FileType::Opus => extract_vorbis_like(tagged.primary_tag()),
        FileType::Aac => extract_universal(tagged.tag(TagType::Id3v2)),
        FileType::Ape => extract_universal(tagged.primary_tag()),
        FileType::Mpc | FileType::WavPack => extract_universal(tagged.primary_tag()),
        _ => extract_universal(tagged.primary_tag()),
    };

    if let Some(pic) = tagged.primary_tag().and_then(|t| t.pictures().first()) {
        if let Some(mime) = pic.mime_type() {
            meta.cover_mime = Some(mime.as_str().to_string());
        }
        meta.cover_data = Some(pic.data().to_vec());
    }

    meta.file_type = file_type;
    meta.raw_size_bytes = raw_size_bytes;
    Ok(meta)
}

fn extract_universal(tag: Option<&Tag>) -> TrackMetadata {
    let mut m = TrackMetadata::default();
    let Some(tag) = tag else { return m };

    m.title = cow_to_string(tag.title());
    m.artist = cow_to_string(tag.artist());
    m.album = cow_to_string(tag.album());
    m.album_artist = tag.get_string(ItemKey::AlbumArtist).map(String::from);
    m.genre = cow_to_string(tag.genre());
    m.composer = tag.get_string(ItemKey::Composer).map(String::from);
    m.publisher = tag.get_string(ItemKey::Publisher).map(String::from);
    m.label = tag.get_string(ItemKey::Label).map(String::from);
    m.encoder = tag.get_string(ItemKey::EncodedBy).map(String::from);
    m.comment = cow_to_string(tag.comment());
    m.lyrics = tag.get_string(ItemKey::Lyrics).map(String::from);

    if let Some(t) = tag.track() {
        m.track_number = Some(t as i64);
    }
    if let Some(t) = tag.track_total() {
        m.track_total = Some(t as i64);
    }
    if let Some(d) = tag.disk() {
        m.disc_number = Some(d as i64);
    }
    if let Some(d) = tag.disk_total() {
        m.disc_total = Some(d as i64);
    }
    if let Some(ts) = tag.date() {
        m.year = Some(ts.year as i64);
    } else if let Some(y) = tag.get_string(ItemKey::Year) {
        if let Ok(n) = y.trim().chars().take(4).collect::<String>().parse::<i64>() {
            m.year = Some(n);
        }
    }
    m
}

fn extract_vorbis_like(tag: Option<&Tag>) -> TrackMetadata {
    let mut m = extract_universal(tag);
    let Some(tag) = tag else { return m };

    m.replay_gain_track_gain_db = parse_gain_db(tag.get_string(ItemKey::ReplayGainTrackGain));
    m.replay_gain_track_peak = parse_double(tag.get_string(ItemKey::ReplayGainTrackPeak));
    m.replay_gain_album_gain_db = parse_gain_db(tag.get_string(ItemKey::ReplayGainAlbumGain));
    m.replay_gain_album_peak = parse_double(tag.get_string(ItemKey::ReplayGainAlbumPeak));
    m
}

fn cow_to_string(v: Option<std::borrow::Cow<'_, str>>) -> Option<String> {
    v.map(|s| s.into_owned())
}

fn parse_gain_db(s: Option<&str>) -> Option<f64> {
    s.and_then(|v| {
        let trimmed = v.trim().trim_end_matches(" dB").trim();
        trimmed.parse::<f64>().ok()
    })
}

fn parse_double(s: Option<&str>) -> Option<f64> {
    s.and_then(|v| v.trim().parse::<f64>().ok())
}

pub fn split_artist_names(raw: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for token in raw.split(|c: char| matches!(c, ',' | ';' | '/') || c == '&' || c == '\n') {
        let cleaned = token
            .trim()
            .trim_start_matches("feat.")
            .trim_start_matches("feat")
            .trim_start_matches("Feat.")
            .trim_start_matches("Feat")
            .trim_start_matches("ft.")
            .trim_start_matches("FT.")
            .trim_start_matches("ft")
            .trim_start_matches("FT")
            .trim_start_matches("with")
            .trim_start_matches("With")
            .trim_start_matches("vs.")
            .trim_start_matches("VS.")
            .trim_start_matches("vs")
            .trim_start_matches("VS")
            .trim()
            .to_string();
        if !cleaned.is_empty() && !out.iter().any(|x| x == &cleaned) {
            out.push(cleaned);
        }
    }
    if out.is_empty() {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            out.push(trimmed.to_string());
        }
    }
    out
}
