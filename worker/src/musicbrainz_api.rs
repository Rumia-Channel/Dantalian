use futures_util::lock::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use std::time::Duration;
use url::Url;
use worker::{Delay, Fetch, Headers, Method, Request, RequestInit, Result};

use crate::external_api::fetch_with_timeout;

const MUSICBRAINZ_MIN_INTERVAL: Duration = Duration::from_millis(1_100);
const MUSICBRAINZ_MAX_RETRIES: usize = 2;

fn musicbrainz_rate_limit_gate() -> &'static Mutex<()> {
    static GATE: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
    &GATE
}

fn musicbrainz_user_agent() -> String {
    format!(
        "Dantalian/{} (https://github.com/Rumia-Channel/dantalian)",
        env!("CARGO_PKG_VERSION")
    )
}

fn musicbrainz_request(url: &Url) -> Result<Request> {
    let headers = Headers::new();
    headers
        .set("User-Agent", &musicbrainz_user_agent())
        .map_err(worker::Error::from)?;
    headers
        .set("Accept", "application/json")
        .map_err(worker::Error::from)?;
    let mut init = RequestInit::new();
    init.with_method(Method::Get).with_headers(headers);
    Request::new_with_init(url.as_str(), &init)
}

async fn fetch_musicbrainz(url: Url, label: &str) -> Result<worker::Response> {
    let _gate = musicbrainz_rate_limit_gate().lock().await;
    let mut retries = 0;
    loop {
        let response =
            fetch_with_timeout(Fetch::Request(musicbrainz_request(&url)?), label).await?;
        let retryable = matches!(response.status_code(), 429 | 503);
        if !retryable || retries >= MUSICBRAINZ_MAX_RETRIES {
            Delay::from(MUSICBRAINZ_MIN_INTERVAL).await;
            return Ok(response);
        }
        retries += 1;
        Delay::from(MUSICBRAINZ_MIN_INTERVAL * retries as u32).await;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MusicBrainzTrack {
    pub disc_number: i64,
    pub track_number: i64,
    pub title: String,
    pub duration: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MusicBrainzCd {
    pub title: String,
    pub artist: Option<String>,
    pub publisher: Option<String>,
    pub label: Option<String>,
    pub catalog_number: Option<String>,
    pub publish_date: Option<String>,
    pub cover_url: Option<String>,
    pub disc_count: Option<i64>,
    pub tracks: Vec<MusicBrainzTrack>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MusicBrainzCandidate {
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub date: Option<String>,
    pub country: Option<String>,
    pub label: Option<String>,
    pub catalog_number: Option<String>,
    pub barcode: Option<String>,
    pub asin: Option<String>,
    pub disc_count: Option<i64>,
    pub track_count: i64,
    pub cover_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MbSearchResponse {
    releases: Option<Vec<MbRelease>>,
}

#[derive(Debug, Deserialize)]
struct MbRelease {
    id: String,
    title: String,
    date: Option<String>,
    country: Option<String>,
    barcode: Option<String>,
    asin: Option<String>,
    #[serde(rename = "artist-credit")]
    artist_credit: Option<Vec<MbArtistCredit>>,
    #[serde(rename = "label-info")]
    label_info: Option<Vec<MbLabelInfo>>,
    media: Option<Vec<MbMedia>>,
    #[serde(rename = "cover-art-archive")]
    cover_art_archive: Option<MbCoverArtArchive>,
}

#[derive(Debug, Deserialize)]
struct MbArtistCredit {
    name: String,
}

#[derive(Debug, Deserialize)]
struct MbLabelInfo {
    label: Option<MbLabel>,
    #[serde(rename = "catalog-number")]
    catalog_number: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MbLabel {
    name: String,
}

#[derive(Debug, Deserialize)]
struct MbMedia {
    position: Option<i64>,
    #[serde(rename = "track-count")]
    track_count: Option<i64>,
    tracks: Option<Vec<MbTrack>>,
}

#[derive(Debug, Deserialize)]
struct MbTrack {
    position: Option<i64>,
    title: String,
    length: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct MbCoverArtArchive {
    front: Option<bool>,
}

fn mb_artist(release: &MbRelease) -> Option<String> {
    release.artist_credit.as_ref().and_then(|credits| {
        let names = credits
            .iter()
            .map(|credit| credit.name.trim())
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>();
        (!names.is_empty()).then(|| names.join(" & "))
    })
}

fn mb_label(release: &MbRelease) -> (Option<String>, Option<String>) {
    release
        .label_info
        .as_ref()
        .and_then(|labels| labels.first())
        .map(|info| {
            (
                info.label.as_ref().map(|label| label.name.clone()),
                info.catalog_number.clone(),
            )
        })
        .unwrap_or((None, None))
}

fn mb_cover_url(release: &MbRelease) -> Option<String> {
    release
        .cover_art_archive
        .as_ref()
        .filter(|archive| archive.front == Some(true))
        .map(|_| format!("https://coverartarchive.org/release/{}/front", release.id))
}

fn format_track_duration(milliseconds: i64) -> String {
    let seconds = milliseconds / 1000;
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

fn mb_tracks(release: &MbRelease) -> Vec<MusicBrainzTrack> {
    release
        .media
        .as_ref()
        .into_iter()
        .flatten()
        .flat_map(|media| {
            let disc_number = media.position.unwrap_or(1).max(1);
            media
                .tracks
                .as_ref()
                .into_iter()
                .flatten()
                .enumerate()
                .map(move |(index, track)| MusicBrainzTrack {
                    disc_number,
                    track_number: track.position.unwrap_or(index as i64 + 1).max(1),
                    title: track.title.clone(),
                    duration: track.length.map(format_track_duration),
                })
        })
        .collect()
}

fn mb_cd(release: MbRelease) -> MusicBrainzCd {
    let (label, catalog_number) = mb_label(&release);
    let artist = mb_artist(&release);
    let cover_url = mb_cover_url(&release);
    let tracks = mb_tracks(&release);
    let disc_count = release
        .media
        .as_ref()
        .map(|media| media.len() as i64)
        .or(Some(1));
    MusicBrainzCd {
        title: release.title,
        artist,
        publisher: label.clone(),
        label,
        catalog_number,
        publish_date: release.date,
        cover_url,
        disc_count,
        tracks,
    }
}

fn mb_candidate(release: MbRelease) -> MusicBrainzCandidate {
    let (label, catalog_number) = mb_label(&release);
    let artist = mb_artist(&release);
    let cover_url = mb_cover_url(&release);
    let disc_count = release.media.as_ref().map(|media| media.len() as i64);
    let track_count = release
        .media
        .as_ref()
        .into_iter()
        .flatten()
        .map(|media| {
            media
                .track_count
                .or_else(|| media.tracks.as_ref().map(|tracks| tracks.len() as i64))
                .unwrap_or(0)
        })
        .sum();
    MusicBrainzCandidate {
        id: release.id.clone(),
        title: release.title,
        artist,
        date: release.date,
        country: release.country,
        label,
        catalog_number,
        barcode: release.barcode.filter(|value| !value.is_empty()),
        asin: release.asin.filter(|value| !value.is_empty()),
        disc_count,
        track_count,
        cover_url,
    }
}

async fn musicbrainz_search(query: &str, limit: usize) -> Result<Vec<MbRelease>> {
    let mut url = Url::parse("https://musicbrainz.org/ws/2/release")
        .map_err(|error| worker::Error::RustError(error.to_string()))?;
    url.query_pairs_mut()
        .append_pair("query", query)
        .append_pair("limit", &limit.clamp(1, 100).to_string())
        .append_pair("fmt", "json");
    let mut response = fetch_musicbrainz(url, "MusicBrainz release search").await?;
    let status = response.status_code();
    if !(200..300).contains(&status) {
        return Err(worker::Error::RustError(format!(
            "MusicBrainz search returned HTTP {status}"
        )));
    }
    let json = response.text().await.map_err(|error| {
        worker::Error::RustError(format!("MusicBrainz search response read failed: {error}"))
    })?;
    serde_json::from_str::<MbSearchResponse>(&json)
        .map(|search| search.releases.unwrap_or_default())
        .map_err(|error| {
            worker::Error::RustError(format!("MusicBrainz search JSON failed: {error}"))
        })
}

async fn musicbrainz_release(release_id: &str) -> Result<Option<MbRelease>> {
    let release_id = release_id.trim();
    if release_id.is_empty()
        || !release_id
            .bytes()
            .all(|value| value.is_ascii_hexdigit() || value == b'-')
    {
        return Ok(None);
    }
    let mut url = Url::parse(&format!(
        "https://musicbrainz.org/ws/2/release/{release_id}"
    ))
    .map_err(|error| worker::Error::RustError(error.to_string()))?;
    url.query_pairs_mut()
        .append_pair("inc", "recordings+artist-credits+labels+media")
        .append_pair("fmt", "json");
    let mut response = fetch_musicbrainz(url, "MusicBrainz release").await?;
    let status = response.status_code();
    if status == 404 {
        return Ok(None);
    }
    if !(200..300).contains(&status) {
        return Err(worker::Error::RustError(format!(
            "MusicBrainz release returned HTTP {status}"
        )));
    }
    let json = response.text().await.map_err(|error| {
        worker::Error::RustError(format!("MusicBrainz release response read failed: {error}"))
    })?;
    serde_json::from_str(&json).map(Some).map_err(|error| {
        worker::Error::RustError(format!("MusicBrainz release JSON failed: {error}"))
    })
}

pub async fn lookup_cd_by_release_id(release_id: &str) -> Result<Option<MusicBrainzCd>> {
    Ok(musicbrainz_release(release_id).await?.map(mb_cd))
}

pub async fn lookup_cd_candidate(release_id: &str) -> Result<Option<MusicBrainzCd>> {
    Delay::from(Duration::from_millis(1100)).await;
    lookup_cd_by_release_id(release_id).await
}

pub async fn lookup_cd_candidates(jan: &str) -> Result<Vec<MusicBrainzCandidate>> {
    let jan: String = jan.chars().filter(|value| value.is_ascii_digit()).collect();
    if jan.len() < 8 || jan.len() > 14 {
        return Err(worker::Error::RustError("invalid JAN length".to_string()));
    }
    let releases = musicbrainz_search(&format!("barcode:{jan}"), 20).await?;
    Ok(releases.into_iter().map(mb_candidate).collect())
}

fn escape_lucene_phrase(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub async fn lookup_cd_candidates_by_title(title: &str) -> Result<Vec<MusicBrainzCandidate>> {
    let title = title.trim();
    if title.is_empty() {
        return Ok(Vec::new());
    }
    let escaped = escape_lucene_phrase(title);
    for query in [
        format!("release:\"{escaped}\""),
        format!("release:{escaped}"),
    ] {
        let releases = musicbrainz_search(&query, 20).await?;
        if !releases.is_empty() {
            return Ok(releases.into_iter().map(mb_candidate).collect());
        }
    }
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::{MbRelease, MusicBrainzCandidate, mb_candidate, mb_cd, musicbrainz_user_agent};

    #[test]
    fn identifies_application_and_contact_in_user_agent() {
        let user_agent = musicbrainz_user_agent();
        assert!(user_agent.starts_with("Dantalian/"));
        assert!(user_agent.contains("(https://github.com/Rumia-Channel/dantalian)"));
    }

    #[test]
    fn parses_musicbrainz_release_fields() {
        let json = r#"{
            "id": "131f6cf7-e5eb-47be-b375-fcc13d1c5c61",
            "title": "Drama CD",
            "date": "2010-01-01",
            "country": "JP",
            "barcode": "4988014634067",
            "asin": "B000000001",
            "artist-credit": [{"name": "Artist"}],
            "label-info": [{"label": {"name": "Label"}, "catalog-number": "CAT-1"}],
            "media": [{
                "position": 1,
                "track-count": 1,
                "tracks": [{"position": 1, "title": "Track 1", "length": 125000}]
            }],
            "cover-art-archive": {"front": true}
        }"#;
        let candidate: MusicBrainzCandidate = mb_candidate(serde_json::from_str(json).unwrap());
        assert_eq!(candidate.title, "Drama CD");
        assert_eq!(candidate.country.as_deref(), Some("JP"));
        assert_eq!(candidate.track_count, 1);
        assert_eq!(candidate.barcode.as_deref(), Some("4988014634067"));
        let release: MbRelease = serde_json::from_str(json).unwrap();
        let cd = mb_cd(release);

        assert_eq!(cd.title, "Drama CD");
        assert_eq!(cd.artist.as_deref(), Some("Artist"));
        assert_eq!(cd.publisher.as_deref(), Some("Label"));
        assert_eq!(cd.catalog_number.as_deref(), Some("CAT-1"));
        assert_eq!(cd.disc_count, Some(1));
        assert_eq!(cd.tracks[0].duration.as_deref(), Some("02:05"));
        assert_eq!(
            cd.cover_url.as_deref(),
            Some("https://coverartarchive.org/release/131f6cf7-e5eb-47be-b375-fcc13d1c5c61/front")
        );
    }
}
