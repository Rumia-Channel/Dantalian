use crate::db::NewTrack;
use crate::db_models::CdInfo;
use reqwest::Client;
use serde::Deserialize;
use std::error::Error as StdError;
use std::sync::OnceLock;
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant, sleep_until};
use tracing::debug;

const MIN_INTERVAL: Duration = Duration::from_millis(1100);

fn rate_limit_gate() -> &'static Mutex<Option<Instant>> {
    static GATE: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    GATE.get_or_init(|| Mutex::new(None))
}

async fn wait_rate_limit() {
    let mut guard = rate_limit_gate().lock().await;
    let now = Instant::now();
    if let Some(last) = *guard {
        let next = last + MIN_INTERVAL;
        if next > now {
            sleep_until(next).await;
        }
    }
    *guard = Some(Instant::now());
}

pub(super) async fn wait_metabrainz_rate_limit() {
    wait_rate_limit().await;
}

fn user_agent(contact: &str) -> String {
    let c = contact.trim();
    let c = if c.is_empty() || c.contains("example.com") {
        "+https://github.com/Rumia-Channel/dantalian"
    } else {
        c
    };
    format!("Dantalian/{} ({})", env!("CARGO_PKG_VERSION"), c)
}

fn format_reqwest_error(prefix: &str, err: reqwest::Error) -> String {
    let mut msg = format!("{}: {}", prefix, err);
    let mut source: Option<&dyn StdError> = err.source();
    while let Some(s) = source {
        msg.push_str(&format!(" -> {}", s));
        source = s.source();
    }
    msg
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
    #[serde(rename = "artist-credit")]
    artist_credit: Option<Vec<MbArtistCredit>>,
    #[serde(rename = "label-info")]
    label_info: Option<Vec<MbLabelInfo>>,
    media: Option<Vec<MbMedia>>,
    #[serde(rename = "cover-art-archive")]
    cover_art_archive: Option<MbCoverArtArchive>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct MbArtistCredit {
    name: String,
    artist: Option<MbArtist>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct MbArtist {
    name: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct MbLabelInfo {
    label: Option<MbLabel>,
    #[serde(rename = "catalog-number")]
    catalog_number: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct MbLabel {
    name: String,
}

#[derive(Debug, Deserialize)]
struct MbCoverArtArchive {
    front: Option<bool>,
    #[allow(dead_code)]
    artwork: Option<bool>,
    #[allow(dead_code)]
    count: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct MbMedia {
    position: Option<i64>,
    #[allow(dead_code)]
    format: Option<String>,
    tracks: Option<Vec<MbTrack>>,
}

#[derive(Debug, Deserialize)]
struct MbTrack {
    position: Option<i64>,
    title: String,
    length: Option<i64>,
}

fn format_duration(ms: i64) -> String {
    let secs = ms / 1000;
    let mins = secs / 60;
    let sec = secs % 60;
    format!("{:02}:{:02}", mins, sec)
}

pub async fn lookup_cd(
    client: &Client,
    jan: &str,
    _images_dir: &str,
    contact: &str,
) -> Result<Option<CdInfo>, String> {
    let clean: String = jan.chars().filter(|c| c.is_ascii_digit()).collect();
    if clean.len() < 8 {
        return Err(format!("Invalid JAN: {}", jan));
    }

    let ua = user_agent(contact);

    let search_url = format!(
        "https://musicbrainz.org/ws/2/release?query=barcode:{}&fmt=json&limit=1",
        clean
    );
    debug!(search_url = %search_url, "MusicBrainz search");

    wait_rate_limit().await;
    let search_resp = client
        .get(&search_url)
        .header("User-Agent", &ua)
        .send()
        .await
        .map_err(|e| format_reqwest_error("HTTP error", e))?;

    if !search_resp.status().is_success() {
        return Err(format!("MusicBrainz returned {}", search_resp.status()));
    }

    let search_body = search_resp
        .bytes()
        .await
        .map_err(|e| format!("Read error: {}", e))?;

    let search: MbSearchResponse =
        serde_json::from_slice(&search_body).map_err(|e| format!("JSON parse: {}", e))?;

    let releases = search.releases.unwrap_or_default();
    if releases.is_empty() {
        return Ok(None);
    }

    let mbid = &releases[0].id;
    let detail_url = format!(
        "https://musicbrainz.org/ws/2/release/{}?inc=recordings+artist-credits+labels+media&fmt=json",
        mbid
    );
    debug!(detail_url = %detail_url, "MusicBrainz detail");

    wait_rate_limit().await;
    let detail_resp = client
        .get(&detail_url)
        .header("User-Agent", &ua)
        .send()
        .await
        .map_err(|e| format_reqwest_error("HTTP error", e))?;

    if !detail_resp.status().is_success() {
        return Err(format!("MusicBrainz detail returned {}", detail_resp.status()));
    }

    let detail_body = detail_resp
        .bytes()
        .await
        .map_err(|e| format!("Read error: {}", e))?;

    let release: MbRelease =
        serde_json::from_slice(&detail_body).map_err(|e| format!("JSON parse: {}", e))?;

    let artist = release
        .artist_credit
        .as_ref()
        .and_then(|ac| {
            let names: Vec<&str> = ac.iter().map(|c| c.name.as_str()).collect();
            if names.is_empty() {
                None
            } else {
                Some(names.join(" & "))
            }
        });

    let (label, catalog_number) = release
        .label_info
        .as_ref()
        .and_then(|li| li.first())
        .map(|li| {
            (
                li.label.as_ref().map(|l| l.name.clone()),
                li.catalog_number.clone(),
            )
        })
        .unwrap_or((None, None));

    let disc_count = release
        .media
        .as_ref()
        .map(|m| m.len() as i64)
        .or(Some(1));

    let mut tracks = Vec::new();
    if let Some(media_list) = &release.media {
        for media in media_list {
            let disc_num = media.position.unwrap_or(1);
            if let Some(track_list) = &media.tracks {
                for track in track_list {
                    tracks.push(NewTrack {
                        disc_number: Some(disc_num),
                        track_number: track.position.unwrap_or(tracks.len() as i64 + 1),
                        title: track.title.clone(),
                        duration: track.length.map(format_duration),
                    });
                }
            }
        }
    }

    let cover_url = match &release.cover_art_archive {
        Some(archive) if archive.front == Some(true) => {
            Some(format!(
                "https://coverartarchive.org/release/{}/front",
                mbid
            ))
        }
        _ => None,
    };

    Ok(Some(CdInfo {
        title: release.title,
        artist,
        publisher: label.clone(),
        label,
        catalog_number,
        publish_date: crate::external::normalize_publish_date(release.date.as_deref()),
        cover_url,
        disc_count,
        tracks,
    }))
}
