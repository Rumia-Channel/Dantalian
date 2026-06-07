use crate::db::NewTrack;
use crate::db_models::CdInfo;
use reqwest::Client;
use serde::Deserialize;
use tracing::debug;

#[derive(Debug, Deserialize)]
struct DcSearchResponse {
    results: Option<Vec<DcSearchResult>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DcSearchResult {
    id: u64,
    title: String,
    year: Option<String>,
    label: Option<Vec<String>>,
    catno: Option<String>,
    thumb: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DcRelease {
    title: String,
    artists: Option<Vec<DcArtist>>,
    labels: Option<Vec<DcLabel>>,
    year: Option<u32>,
    tracklist: Option<Vec<DcTrack>>,
    images: Option<Vec<DcImage>>,
    formats: Option<Vec<DcFormat>>,
}

#[derive(Debug, Deserialize)]
struct DcArtist {
    name: String,
}

#[derive(Debug, Deserialize)]
struct DcLabel {
    name: String,
    catno: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DcTrack {
    position: Option<String>,
    title: String,
    duration: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DcImage {
    uri: Option<String>,
    #[serde(rename = "type")]
    image_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DcFormat {
    name: Option<String>,
    descriptions: Option<Vec<String>>,
}

fn parse_discogs_position(pos: &str) -> (Option<i64>, i64) {
    if let Some((d, t)) = pos.split_once('-') {
        let disc = d.trim().parse::<i64>().ok();
        let track = t.trim().parse::<i64>().unwrap_or(1);
        (disc, track)
    } else {
        let track = pos.trim().parse::<i64>().unwrap_or(1);
        (None, track)
    }
}

fn normalize_duration(dur: &str) -> Option<String> {
    let dur = dur.trim();
    if dur.is_empty() {
        return None;
    }
    if dur.contains(':') {
        return Some(dur.to_string());
    }
    if let Ok(secs) = dur.parse::<i64>() {
        let mins = secs / 60;
        let sec = secs % 60;
        return Some(format!("{:02}:{:02}", mins, sec));
    }
    None
}

pub async fn lookup_cd_discogs(
    client: &Client,
    jan: &str,
    token: &str,
) -> Result<Option<CdInfo>, String> {
    let clean: String = jan.chars().filter(|c| c.is_ascii_digit()).collect();

    let search_url = format!(
        "https://api.discogs.com/database/search?barcode={}&type=release&per_page=1",
        clean
    );
    debug!(search_url = %search_url, "Discogs search");

    let ua = format!("Dantalian/{}", env!("CARGO_PKG_VERSION"));
    let search_resp = client
        .get(&search_url)
        .header("User-Agent", &ua)
        .header("Authorization", format!("Discogs token={}", token))
        .send()
        .await
        .map_err(|e| format!("Discogs HTTP error: {}", e))?;

    if !search_resp.status().is_success() {
        return Err(format!("Discogs returned {}", search_resp.status()));
    }

    let search: DcSearchResponse = search_resp
        .json()
        .await
        .map_err(|e| format!("Discogs JSON parse: {}", e))?;

    let results = search.results.unwrap_or_default();
    if results.is_empty() {
        return Ok(None);
    }

    let release_id = results[0].id;
    let detail_url = format!("https://api.discogs.com/releases/{}", release_id);
    debug!(detail_url = %detail_url, "Discogs release detail");

    let detail_resp = client
        .get(&detail_url)
        .header("User-Agent", &ua)
        .header("Authorization", format!("Discogs token={}", token))
        .send()
        .await
        .map_err(|e| format!("Discogs detail HTTP error: {}", e))?;

    if !detail_resp.status().is_success() {
        return Err(format!("Discogs detail returned {}", detail_resp.status()));
    }

    let release: DcRelease = detail_resp
        .json()
        .await
        .map_err(|e| format!("Discogs detail JSON parse: {}", e))?;

    let artist = release
        .artists
        .as_ref()
        .and_then(|a| {
            let names: Vec<&str> = a.iter().map(|x| x.name.as_str()).collect();
            if names.is_empty() { None } else { Some(names.join(" & ")) }
        });

    let default_disc_count: Option<i64> = release.formats.as_ref().and_then(|fmts| {
        fmts.iter().find_map(|f| {
            f.descriptions.as_ref().and_then(|descs| {
                descs.iter().find_map(|d| {
                    if let Some(pos) = d.find("枚組") {
                        d[..pos].trim().parse::<i64>().ok()
                    } else {
                        None
                    }
                })
            })
        })
    });

    let (label, catalog_number) = release
        .labels
        .as_ref()
        .and_then(|l| l.first())
        .map(|l| (Some(l.name.clone()), l.catno.clone()))
        .unwrap_or((None, None));

    let mut tracks: Vec<NewTrack> = Vec::new();
    let mut max_disc = 1i64;

    if let Some(tracklist) = &release.tracklist {
        for (i, t) in tracklist.iter().enumerate() {
            let (disc_num, track_num) = if let Some(ref pos) = t.position {
                parse_discogs_position(pos)
            } else {
                (Some(1), (i + 1) as i64)
            };
            let dnum = disc_num.unwrap_or(1);
            if dnum > max_disc {
                max_disc = dnum;
            }
            tracks.push(NewTrack {
                disc_number: Some(dnum),
                track_number: track_num,
                title: t.title.clone(),
                duration: t.duration.as_deref().and_then(normalize_duration),
            });
        }
    }

    let disc_count = default_disc_count.or(Some(max_disc));

    let cover_url = release.images.as_ref().and_then(|imgs| {
        imgs.iter()
            .filter(|img| img.image_type.as_deref() == Some("primary"))
            .find_map(|img| img.uri.clone())
            .or_else(|| imgs.first().and_then(|img| img.uri.clone()))
    });

    let publish_date = release
        .year
        .map(|y| crate::external::normalize_publish_date(Some(&y.to_string())).unwrap_or_default());

    Ok(Some(CdInfo {
        title: release.title,
        artist,
        publisher: label.clone(),
        label,
        catalog_number,
        publish_date,
        cover_url,
        disc_count,
        tracks,
    }))
}
