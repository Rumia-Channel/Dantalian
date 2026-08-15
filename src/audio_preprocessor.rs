use std::collections::BTreeMap;
use std::io::Cursor;

use serde::Serialize;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, RawValue, Tag};

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AudioPreprocessorMetadata {
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
    pub replay_gain_track_gain_db: Option<f64>,
    pub replay_gain_track_peak: Option<f64>,
    pub replay_gain_album_gain_db: Option<f64>,
    pub replay_gain_album_peak: Option<f64>,
    pub file_type: Option<String>,
    pub raw_size_bytes: Option<i64>,
    pub duration_seconds: Option<f64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub bitrate_kbps: Option<u32>,
    pub tags: BTreeMap<String, String>,
}

/// Parse audio tags and technical metadata without writing to the filesystem.
///
/// The caller owns the input bytes. This function intentionally performs only
/// bounded metadata extraction and frame counting; it never encodes or uploads
/// audio and is therefore suitable for a browser WASM preflight.
pub fn inspect(source: &[u8], source_extension: &str) -> Result<AudioPreprocessorMetadata, String> {
    if source.is_empty() {
        return Err("audio input is empty".to_string());
    }
    let extension = normalize_extension(source_extension)
        .ok_or_else(|| "invalid audio source extension".to_string())?;

    let mut hint = Hint::new();
    hint.with_extension(&extension);
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            MediaSourceStream::new(Box::new(Cursor::new(source)), Default::default()),
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|error| format!("audio format probe failed: {error}"))?;

    let tags = format
        .metadata()
        .current()
        .map(|revision| revision.media.tags.clone())
        .unwrap_or_default();
    let tags = collect_tags(&tags);

    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| "audio track not found".to_string())?;
    let track_id = track.id;
    let codec_params = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
        .ok_or_else(|| "audio codec parameters not found".to_string())?;
    let mut sample_rate = codec_params.sample_rate.unwrap_or_default();
    let mut channels = codec_params
        .channels
        .as_ref()
        .map(|value| value.count())
        .filter(|count| *count > 0)
        .and_then(|count| u8::try_from(count).ok());
    let declared_frames = track.num_frames.unwrap_or_default();
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(codec_params, &AudioDecoderOptions::default())
        .map_err(|error| format!("audio decoder creation failed: {error}"))?;

    let mut decoded_frames = 0_u64;
    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) | Err(SymphoniaError::ResetRequired) => break,
            Err(error) => return Err(format!("audio packet read failed: {error}")),
        };
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                sample_rate = decoded.spec().rate();
                channels = u8::try_from(decoded.spec().channels().count()).ok();
                decoded_frames = decoded_frames.saturating_add(decoded.frames() as u64);
            }
            Err(SymphoniaError::DecodeError(_)) | Err(SymphoniaError::IoError(_)) => continue,
            Err(error) => return Err(format!("audio decode failed: {error}")),
        }
    }

    let total_frames = if decoded_frames > 0 {
        decoded_frames
    } else {
        declared_frames
    };
    if total_frames == 0 {
        return Err("audio contains no decodable frames".to_string());
    }
    if sample_rate == 0 {
        return Err("audio sample rate is unavailable".to_string());
    }

    let duration_seconds = total_frames as f64 / f64::from(sample_rate);
    let bitrate_kbps = if duration_seconds > 0.0 {
        u32::try_from(((source.len() as f64 * 8.0) / duration_seconds / 1_000.0).round() as u64)
            .ok()
            .filter(|bit_rate| *bit_rate > 0)
    } else {
        None
    };

    Ok(AudioPreprocessorMetadata {
        title: first_tag(&tags, &["title", "tracktitle", "tit2"]),
        artist: first_tag(&tags, &["artist", "performer", "trackartist", "tpe1"]),
        album: first_tag(&tags, &["album", "talb"]),
        album_artist: first_tag(&tags, &["albumartist", "album_artist", "tpe2"]),
        track_number: first_tag(&tags, &["tracknumber", "track", "trck"])
            .and_then(|value| parse_index(&value)),
        track_total: first_tag(&tags, &["tracktotal", "totaltracks"])
            .and_then(|value| parse_index(&value)),
        disc_number: first_tag(&tags, &["discnumber", "disc", "tpos"])
            .and_then(|value| parse_index(&value)),
        disc_total: first_tag(&tags, &["disctotal", "totaldiscs"])
            .and_then(|value| parse_index(&value)),
        year: first_tag(&tags, &["year", "date", "tdrc"]).and_then(|value| parse_year(&value)),
        genre: first_tag(&tags, &["genre", "tcon"]),
        composer: first_tag(&tags, &["composer", "tcom"]),
        publisher: first_tag(&tags, &["publisher", "tpub"]),
        label: first_tag(&tags, &["label"]),
        encoder: first_tag(&tags, &["encoder", "encodedby", "tenc"]),
        comment: first_tag(&tags, &["comment", "description", "comm"]),
        lyrics: first_tag(&tags, &["lyrics", "unsyncedlyrics", "uslt"]),
        replay_gain_track_gain_db: first_tag(
            &tags,
            &["replaygaintrackgain", "replay_gain_track_gain"],
        )
        .and_then(|value| parse_float(&value)),
        replay_gain_track_peak: first_tag(
            &tags,
            &["replaygaintrackpeak", "replay_gain_track_peak"],
        )
        .and_then(|value| parse_float(&value)),
        replay_gain_album_gain_db: first_tag(
            &tags,
            &["replaygainalbumgain", "replay_gain_album_gain"],
        )
        .and_then(|value| parse_float(&value)),
        replay_gain_album_peak: first_tag(
            &tags,
            &["replaygainalbumpeak", "replay_gain_album_peak"],
        )
        .and_then(|value| parse_float(&value)),
        file_type: Some(extension),
        raw_size_bytes: i64::try_from(source.len()).ok(),
        duration_seconds: Some(duration_seconds),
        sample_rate: Some(sample_rate),
        channels,
        bitrate_kbps,
        tags,
    })
}

pub fn normalize_extension(value: &str) -> Option<String> {
    let extension = value.trim().trim_start_matches('.').to_ascii_lowercase();
    if extension.is_empty()
        || extension.len() > 10
        || !extension
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        return None;
    }
    Some(extension)
}

fn collect_tags(tags: &[Tag]) -> BTreeMap<String, String> {
    let mut collected = BTreeMap::new();
    for tag in tags {
        let Some(value) = raw_value_to_string(&tag.raw.value) else {
            continue;
        };
        let key = normalize_tag_key(&tag.raw.key);
        if !key.is_empty() {
            collected.entry(key).or_insert(value);
        }
    }
    collected
}

fn raw_value_to_string(value: &RawValue) -> Option<String> {
    match value {
        RawValue::Binary(_) => None,
        RawValue::Boolean(value) => Some(value.to_string()),
        RawValue::Flag => Some("true".to_string()),
        RawValue::Float(value) => Some(value.to_string()),
        RawValue::SignedInt(value) => Some(value.to_string()),
        RawValue::String(value) => Some(value.to_string()),
        RawValue::StringList(values) => Some(
            values
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join("; "),
        ),
        RawValue::UnsignedInt(value) => Some(value.to_string()),
        _ => None,
    }
}

fn normalize_tag_key(key: &str) -> String {
    key.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

fn first_tag(tags: &BTreeMap<String, String>, aliases: &[&str]) -> Option<String> {
    aliases
        .iter()
        .map(|alias| normalize_tag_key(alias))
        .find_map(|alias| tags.get(&alias).cloned())
        .filter(|value| !value.trim().is_empty())
}

fn parse_index(value: &str) -> Option<i64> {
    value
        .split(['/', '\\'])
        .next()
        .and_then(|part| part.trim().parse::<i64>().ok())
}

fn parse_year(value: &str) -> Option<i64> {
    value
        .split(['-', '/', 'T'])
        .next()
        .and_then(|part| part.trim().parse::<i64>().ok())
}

fn parse_float(value: &str) -> Option<f64> {
    value
        .trim()
        .trim_end_matches(|character: char| character.is_ascii_alphabetic())
        .trim()
        .parse::<f64>()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::inspect;

    #[test]
    fn extracts_pcm_audio_technical_metadata() {
        let source = pcm_wav(8_000, 2, 800);
        let metadata = inspect(&source, "wav").expect("WAV metadata");

        assert_eq!(metadata.file_type.as_deref(), Some("wav"));
        assert_eq!(metadata.raw_size_bytes, Some(source.len() as i64));
        assert_eq!(metadata.sample_rate, Some(8_000));
        assert_eq!(metadata.channels, Some(2));
        assert_eq!(metadata.duration_seconds, Some(0.1));
    }

    #[test]
    fn rejects_invalid_input_and_extensions() {
        assert!(inspect(&[], "wav").is_err());
        assert!(inspect(b"not audio", "../wav").is_err());
        assert!(inspect(b"not audio", "wav").is_err());
    }

    fn pcm_wav(sample_rate: u32, channels: u16, frames: usize) -> Vec<u8> {
        let data_len = frames * usize::from(channels) * 2;
        let byte_rate = sample_rate * u32::from(channels) * 2;
        let block_align = channels * 2;
        let mut wav = Vec::with_capacity(44 + data_len);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data_len as u32).to_le_bytes());
        wav.resize(44 + data_len, 0);
        wav
    }
}
