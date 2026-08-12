use crate::db::Db;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use tokio::sync::Notify;

pub const KEY_ENABLED: &str = "audio.data_saver.enabled";
pub const KEY_EXTENSIONS: &str = "audio.data_saver.extensions";
pub const DEFAULT_EXTENSIONS: &str = "wav,flac,aiff,alac";

const TARGET_BITRATE: u32 = 192_000;
const BACKGROUND_SCAN_INTERVAL: Duration = Duration::from_secs(60);
const BACKGROUND_RETRY_INTERVAL: Duration = Duration::from_secs(300);
const BACKGROUND_TRACK_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
pub struct AudioDataSaverConfig {
    pub enabled: bool,
    pub extensions: HashSet<String>,
}

impl AudioDataSaverConfig {
    pub fn load(db: &Db) -> Self {
        let enabled = db
            .get_setting(KEY_ENABLED)
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "on"
                )
            })
            .unwrap_or(false);
        let extensions = db
            .get_setting(KEY_EXTENSIONS)
            .unwrap_or_else(|| DEFAULT_EXTENSIONS.to_string())
            .split(',')
            .filter_map(normalize_extension)
            .collect();
        Self {
            enabled,
            extensions,
        }
    }

    pub fn applies_to(&self, extension: &str) -> bool {
        self.enabled
            && normalize_extension(extension)
                .is_some_and(|extension| self.extensions.contains(&extension))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EncodedVariants {
    pub opus: bool,
    pub aac: bool,
}

#[derive(Debug, Clone, Copy)]
struct AudioProfile {
    source_rate: u32,
    target_rate: u32,
    channels: u8,
}

#[derive(Debug)]
struct DecodedAudio {
    sample_rate: u32,
    samples: Vec<i16>,
}

static ENCODE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn encode_lock() -> &'static Mutex<()> {
    ENCODE_LOCK.get_or_init(|| Mutex::new(()))
}

pub fn start_background_worker(
    db: Db,
    audio_dir: String,
    notify: Arc<Notify>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut retry_after = HashMap::<String, Instant>::new();

        loop {
            let config = AudioDataSaverConfig::load(&db);
            if !config.enabled || config.extensions.is_empty() {
                wait_for_wakeup(&notify, BACKGROUND_SCAN_INTERVAL).await;
                continue;
            }

            let now = Instant::now();
            retry_after.retain(|_, until| *until > now);
            let sources = match db.list_audio_encoding_sources() {
                Ok(sources) => sources,
                Err(error) => {
                    tracing::warn!("Background audio encoder could not list tracks: {}", error);
                    wait_for_wakeup(&notify, BACKGROUND_SCAN_INTERVAL).await;
                    continue;
                }
            };

            let mut attempted = false;
            let mut seen_hashes = HashSet::new();
            for (file_hash, file_name) in sources {
                if !seen_hashes.insert(file_hash.clone()) {
                    continue;
                }
                let Some(source_extension) = source_extension(&file_name, &file_hash) else {
                    continue;
                };
                if !config.applies_to(&source_extension)
                    || retry_after
                        .get(&file_hash)
                        .is_some_and(|until| *until > Instant::now())
                    || encoded_variants_exist(&audio_dir, &file_hash)
                {
                    continue;
                }

                attempted = true;
                let worker_audio_dir = audio_dir.clone();
                let worker_file_hash = file_hash.clone();
                let worker_extension = source_extension.clone();
                tracing::info!(
                    file_hash = %file_hash,
                    source_extension = %source_extension,
                    "Background audio encoding started"
                );
                let result = tokio::task::spawn_blocking(move || {
                    ensure_encoded_variants(&worker_audio_dir, &worker_file_hash, &worker_extension)
                })
                .await;

                match result {
                    Ok(Ok(variants)) if variants.opus && variants.aac => {
                        tracing::info!(file_hash = %file_hash, "Background audio encoding completed");
                    }
                    Ok(Ok(variants)) => {
                        tracing::warn!(
                            file_hash = %file_hash,
                            opus = variants.opus,
                            aac = variants.aac,
                            "Background audio encoding completed partially; will retry the missing variant"
                        );
                        retry_after.insert(
                            file_hash.clone(),
                            Instant::now() + BACKGROUND_RETRY_INTERVAL,
                        );
                    }
                    Ok(Err(error)) => {
                        tracing::warn!(
                            file_hash = %file_hash,
                            "Background audio encoding failed: {}",
                            error
                        );
                        retry_after.insert(
                            file_hash.clone(),
                            Instant::now() + BACKGROUND_RETRY_INTERVAL,
                        );
                    }
                    Err(error) => {
                        tracing::warn!(
                            file_hash = %file_hash,
                            "Background audio encoding task failed: {}",
                            error
                        );
                        retry_after.insert(
                            file_hash.clone(),
                            Instant::now() + BACKGROUND_RETRY_INTERVAL,
                        );
                    }
                }

                tokio::time::sleep(BACKGROUND_TRACK_INTERVAL).await;
                if !AudioDataSaverConfig::load(&db).enabled {
                    break;
                }
            }

            let wait = if attempted {
                BACKGROUND_TRACK_INTERVAL
            } else {
                BACKGROUND_SCAN_INTERVAL
            };
            wait_for_wakeup(&notify, wait).await;
        }
    })
}

async fn wait_for_wakeup(notify: &Notify, timeout: Duration) {
    tokio::select! {
        _ = notify.notified() => {}
        _ = tokio::time::sleep(timeout) => {}
    }
}

fn source_extension(file_name: &str, file_hash: &str) -> Option<String> {
    Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .and_then(normalize_extension)
        .or_else(|| {
            Path::new(file_hash)
                .extension()
                .and_then(|value| value.to_str())
                .and_then(normalize_extension)
        })
}

fn encoded_variants_exist(audio_dir: &str, file_hash: &str) -> bool {
    encoded_path(audio_dir, file_hash, "opus").is_file()
        && encoded_path(audio_dir, file_hash, "aac").is_file()
}

pub fn normalize_extension(value: &str) -> Option<String> {
    let extension = value.trim().trim_start_matches('.').to_ascii_lowercase();
    if extension.is_empty()
        || extension.len() > 10
        || !extension.chars().all(|ch| ch.is_ascii_alphanumeric())
    {
        return None;
    }
    Some(extension)
}

pub fn is_safe_hash(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.len() <= 256
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

pub fn encoded_path(audio_dir: &str, file_hash: &str, format: &str) -> PathBuf {
    Path::new(audio_dir)
        .join("encoded")
        .join(format)
        .join(encoded_file_name(file_hash, format))
}

pub fn encoded_file_name(file_hash: &str, format: &str) -> String {
    let hash_stem = file_hash
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(file_hash);
    format!("{}.{}", hash_stem, format)
}

pub fn ensure_encoded_variants(
    audio_dir: &str,
    file_hash: &str,
    source_extension: &str,
) -> Result<EncodedVariants, String> {
    if !is_safe_hash(file_hash) {
        return Err("Invalid audio file hash".to_string());
    }
    let source_extension = normalize_extension(source_extension)
        .ok_or_else(|| "Invalid audio source extension".to_string())?;
    let source_path = Path::new(audio_dir).join(file_hash);
    if !source_path.is_file() {
        return Err("Original audio file not found".to_string());
    }

    let _guard = encode_lock()
        .lock()
        .map_err(|_| "Audio encoder lock is poisoned".to_string())?;
    let opus_path = encoded_path(audio_dir, file_hash, "opus");
    let aac_path = encoded_path(audio_dir, file_hash, "aac");
    fs::create_dir_all(opus_path.parent().unwrap()).map_err(|error| error.to_string())?;
    fs::create_dir_all(aac_path.parent().unwrap()).map_err(|error| error.to_string())?;

    let mut variants = EncodedVariants {
        opus: opus_path.is_file(),
        aac: aac_path.is_file(),
    };
    if variants.opus && variants.aac {
        return Ok(variants);
    }

    let profile = probe_audio(&source_path, &source_extension)?;
    if !variants.opus {
        match encode_opus(&source_path, &opus_path, &source_extension, profile) {
            Ok(()) => variants.opus = true,
            Err(error) => tracing::warn!(file_hash, "Opus generation failed: {}", error),
        }
    }
    if !variants.aac {
        match encode_aac(&source_path, &aac_path, &source_extension, profile) {
            Ok(()) => variants.aac = true,
            Err(error) => tracing::warn!(file_hash, "AAC generation failed: {}", error),
        }
    }

    if !variants.opus && !variants.aac {
        return Err(format!(
            "Could not generate a data-saver variant for .{}",
            source_extension
        ));
    }
    Ok(variants)
}

fn probe_audio(source_path: &Path, source_extension: &str) -> Result<AudioProfile, String> {
    let file = File::open(source_path).map_err(|error| format!("audio open failed: {}", error))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(source_extension);
    let format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|error| format!("audio format probe failed: {}", error))?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| "audio track not found".to_string())?;
    let codec_params = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
        .ok_or_else(|| "audio codec parameters not found".to_string())?;
    let source_rate = codec_params.sample_rate.unwrap_or(48_000);
    let channels = codec_params
        .channels
        .as_ref()
        .map(|value| value.count())
        .unwrap_or(2)
        .clamp(1, 2) as u8;
    Ok(AudioProfile {
        source_rate,
        target_rate: opus_sample_rate(source_rate),
        channels,
    })
}

fn opus_sample_rate(source_rate: u32) -> u32 {
    [8_000, 12_000, 16_000, 24_000, 48_000]
        .into_iter()
        .find(|target| source_rate <= *target)
        .unwrap_or(48_000)
}

fn decode_audio(source_path: &Path, source_extension: &str) -> Result<DecodedAudio, String> {
    let file = File::open(source_path).map_err(|error| format!("audio open failed: {}", error))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(source_extension);
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|error| format!("audio format probe failed: {}", error))?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| "audio track not found".to_string())?;
    let track_id = track.id;
    let codec_params = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
        .ok_or_else(|| "audio codec parameters not found".to_string())?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(codec_params, &AudioDecoderOptions::default())
        .map_err(|error| format!("audio decoder creation failed: {}", error))?;

    let mut sample_rate = codec_params.sample_rate.unwrap_or(48_000);
    let mut samples = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) | Err(SymphoniaError::ResetRequired) => break,
            Err(error) => return Err(format!("audio packet read failed: {}", error)),
        };
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                sample_rate = decoded.spec().rate();
                let decoded_channels = decoded.spec().channels().count();
                let target_channels = decoded_channels.clamp(1, 2);
                let mut packet_samples = Vec::new();
                decoded.copy_to_vec_interleaved::<i16>(&mut packet_samples);
                samples.extend(normalize_channels(
                    &packet_samples,
                    decoded_channels,
                    target_channels,
                ));
            }
            Err(SymphoniaError::DecodeError(_)) | Err(SymphoniaError::IoError(_)) => continue,
            Err(error) => return Err(format!("audio decode failed: {}", error)),
        }
    }
    if samples.is_empty() {
        return Err("audio contains no decodable samples".to_string());
    }
    Ok(DecodedAudio {
        sample_rate,
        samples,
    })
}

fn normalize_channels(samples: &[i16], source_channels: usize, target_channels: usize) -> Vec<i16> {
    if source_channels == target_channels {
        return samples.to_vec();
    }
    let frames = samples.len() / source_channels.max(1);
    let mut output = Vec::with_capacity(frames * target_channels);
    for frame in samples.chunks_exact(source_channels.max(1)) {
        match target_channels {
            1 => {
                let sum: i64 = frame.iter().map(|sample| i64::from(*sample)).sum();
                output.push(
                    (sum / frame.len() as i64).clamp(i64::from(i16::MIN), i64::from(i16::MAX))
                        as i16,
                );
            }
            _ => {
                output.push(frame[0]);
                output.push(*frame.get(1).unwrap_or(&frame[0]));
            }
        }
    }
    output
}

fn resample_audio(decoded: DecodedAudio, profile: AudioProfile) -> Vec<i16> {
    if decoded.sample_rate == profile.target_rate {
        return decoded.samples;
    }
    let channels = usize::from(profile.channels);
    let input_frames = decoded.samples.len() / channels;
    let output_frames = ((input_frames as u64 * u64::from(profile.target_rate)
        + u64::from(decoded.sample_rate)
        - 1)
        / u64::from(decoded.sample_rate)) as usize;
    let mut output = Vec::with_capacity(output_frames * channels);
    for output_frame in 0..output_frames {
        let source_position =
            output_frame as f64 * f64::from(decoded.sample_rate) / f64::from(profile.target_rate);
        let source_index = source_position.floor() as usize;
        let fraction = source_position.fract();
        let first = source_index.min(input_frames.saturating_sub(1));
        let second = (first + 1).min(input_frames.saturating_sub(1));
        for channel in 0..channels {
            let a = f64::from(decoded.samples[first * channels + channel]);
            let b = f64::from(decoded.samples[second * channels + channel]);
            output.push((a + (b - a) * fraction).round().clamp(-32768.0, 32767.0) as i16);
        }
    }
    output
}

fn encode_opus(
    source_path: &Path,
    target_path: &Path,
    source_extension: &str,
    profile: AudioProfile,
) -> Result<(), String> {
    use ogg::{PacketWriteEndInfo, PacketWriter};
    use opus_rs::{Application, OpusEncoder};
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let decoded = decode_audio(source_path, source_extension)?;
    let pcm = resample_audio(decoded, profile);
    let mut encoder = OpusEncoder::new(
        profile.target_rate as i32,
        usize::from(profile.channels),
        Application::Audio,
    )
    .map_err(|error| format!("Opus encoder creation failed: {}", error))?;
    encoder.bitrate_bps = TARGET_BITRATE as i32;
    encoder.use_cbr = false;

    let frame_samples = (profile.target_rate / 50) as usize;
    let frame_len = frame_samples * usize::from(profile.channels);
    let mut packets = Vec::new();
    for chunk in pcm.chunks(frame_len) {
        let mut frame = vec![0.0_f32; frame_len];
        for (destination, sample) in frame.iter_mut().zip(chunk) {
            *destination = f32::from(*sample) / 32_768.0;
        }
        let mut output = vec![0_u8; 4096];
        let encoded_len = encoder
            .encode(&frame, frame_samples, &mut output)
            .map_err(|error| format!("Opus encode failed: {}", error))?;
        output.truncate(encoded_len);
        packets.push(output);
    }
    if packets.is_empty() {
        return Err("Opus encoder produced no packets".to_string());
    }

    // opus-rs does not expose the encoder lookahead. 312 samples is the
    // standard 48 kHz Opus pre-skip used by the reference encoder.
    const OPUS_LOOKAHEAD_48K: u64 = 312;
    let lookahead_48k = OPUS_LOOKAHEAD_48K;
    let pre_skip = (lookahead_48k * u64::from(profile.target_rate) / 48_000) as u16;
    let mut hasher = DefaultHasher::new();
    target_path.to_string_lossy().hash(&mut hasher);
    let serial = hasher.finish() as u32;
    let temporary_path = temporary_path(target_path);
    let file = File::create(&temporary_path).map_err(|error| error.to_string())?;
    let packet_count = packets.len();
    let mut writer = PacketWriter::new(BufWriter::new(file));
    writer
        .write_packet(
            opus_head(profile.source_rate, profile.channels, pre_skip),
            serial,
            PacketWriteEndInfo::EndPage,
            0,
        )
        .map_err(|error| format!("Ogg Opus header write failed: {}", error))?;
    writer
        .write_packet(opus_tags(), serial, PacketWriteEndInfo::EndPage, 0)
        .map_err(|error| format!("Ogg Opus tags write failed: {}", error))?;

    let frame_samples = frame_samples as u64;
    let pcm_frames = (pcm.len() / usize::from(profile.channels)) as u64;
    let final_granule_position = lookahead_48k
        + (pcm_frames * 48_000 + u64::from(profile.target_rate) - 1)
            / u64::from(profile.target_rate);
    let mut granule_position = lookahead_48k;
    for (index, packet) in packets.into_iter().enumerate() {
        granule_position = if index + 1 == packet_count {
            final_granule_position
        } else {
            granule_position + frame_samples * 48_000 / u64::from(profile.target_rate)
        };
        let end_info = if index + 1 == packet_count {
            PacketWriteEndInfo::EndStream
        } else {
            PacketWriteEndInfo::NormalPacket
        };
        writer
            .write_packet(packet, serial, end_info, granule_position)
            .map_err(|error| format!("Ogg Opus packet write failed: {}", error))?;
    }
    let mut file = writer.into_inner();
    file.flush().map_err(|error| error.to_string())?;
    drop(file);
    publish_temporary_file(&temporary_path, target_path)
}

fn opus_head(input_rate: u32, channels: u8, pre_skip: u16) -> Vec<u8> {
    let mut header = Vec::with_capacity(19);
    header.extend_from_slice(b"OpusHead");
    header.extend_from_slice(&[1, channels]);
    header.extend_from_slice(&pre_skip.to_le_bytes());
    header.extend_from_slice(&input_rate.to_le_bytes());
    header.extend_from_slice(&0_i16.to_le_bytes());
    header.push(0);
    header
}

fn opus_tags() -> Vec<u8> {
    let vendor = b"Dantalian";
    let mut tags = Vec::with_capacity(16 + vendor.len());
    tags.extend_from_slice(b"OpusTags");
    tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    tags.extend_from_slice(vendor);
    tags.extend_from_slice(&0_u32.to_le_bytes());
    tags
}

fn encode_aac(
    source_path: &Path,
    target_path: &Path,
    source_extension: &str,
    profile: AudioProfile,
) -> Result<(), String> {
    encode_aac_with_rust(source_path, target_path, source_extension, profile)
}
fn encode_aac_with_rust(
    source_path: &Path,
    target_path: &Path,
    source_extension: &str,
    profile: AudioProfile,
) -> Result<(), String> {
    use fdk_aac_rust::encoder::{
        ConfiguredPureRustEncoder, EncoderParameter, PureRustEncoderParameters,
    };

    let decoded = decode_audio(source_path, source_extension)?;
    let pcm = resample_audio(decoded, profile);
    let mut parameters = PureRustEncoderParameters::new(2);
    parameters
        .set_parameter(EncoderParameter::AudioObjectType, 2)
        .map_err(|error| format!("invalid AAC audio object type: {:?}", error))?;
    parameters
        .set_parameter(EncoderParameter::ChannelMode, u32::from(profile.channels))
        .map_err(|error| format!("invalid AAC channel mode: {:?}", error))?;
    parameters
        .set_parameter(EncoderParameter::SampleRate, profile.target_rate)
        .map_err(|error| format!("invalid AAC sample rate: {:?}", error))?;
    parameters
        .set_parameter(EncoderParameter::Bitrate, TARGET_BITRATE)
        .map_err(|error| format!("invalid AAC bitrate: {:?}", error))?;
    parameters
        .set_parameter(EncoderParameter::TransportMux, 2)
        .map_err(|error| format!("invalid AAC transport: {:?}", error))?;
    let mut encoder = ConfiguredPureRustEncoder::from_parameters(&parameters)
        .map_err(|error| format!("could not initialize AAC encoder: {:?}", error))?;

    let frame_len = encoder.input_samples_per_channel() * usize::from(profile.channels);
    let temporary_path = temporary_path(target_path);
    let output_file = File::create(&temporary_path).map_err(|error| error.to_string())?;
    let mut writer = BufWriter::new(output_file);
    let mut encoded_frames = 0;
    for chunk in pcm.chunks(frame_len) {
        let mut frame = vec![0.0_f32; frame_len];
        for (destination, sample) in frame.iter_mut().zip(chunk) {
            *destination = f32::from(*sample) / 32_768.0;
        }
        let encoded = encoder
            .encode_transport_f32(&frame)
            .map_err(|error| format!("AAC encode failed: {:?}", error))?;
        if !encoded.is_empty() {
            writer
                .write_all(&encoded)
                .map_err(|error| error.to_string())?;
            encoded_frames += 1;
        }
    }
    if encoded_frames == 0 {
        return Err("AAC encoder produced no frames".to_string());
    }
    writer.flush().map_err(|error| error.to_string())?;
    drop(writer);
    publish_temporary_file(&temporary_path, target_path)
}

fn temporary_path(target_path: &Path) -> PathBuf {
    target_path.with_file_name(format!(
        ".{}.{}.tmp",
        target_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("audio"),
        std::process::id()
    ))
}

fn publish_temporary_file(temporary_path: &Path, target_path: &Path) -> Result<(), String> {
    if !temporary_path.is_file() {
        return Err("Encoder did not create an output file".to_string());
    }
    fs::rename(temporary_path, target_path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        AudioDataSaverConfig, AudioProfile, encode_aac_with_rust, encode_opus, encoded_file_name,
        is_safe_hash, normalize_extension, opus_sample_rate, source_extension,
    };
    use crate::db::Db;

    #[test]
    fn data_saver_is_disabled_when_setting_is_missing() {
        let db = Db::new(":memory:").expect("database");
        let config = AudioDataSaverConfig::load(&db);

        assert!(!config.enabled);
        assert!(!config.applies_to("wav"));
    }

    #[test]
    fn chooses_the_smallest_supported_opus_rate_not_below_source() {
        assert_eq!(opus_sample_rate(22_050), 24_000);
        assert_eq!(opus_sample_rate(44_100), 48_000);
        assert_eq!(opus_sample_rate(96_000), 48_000);
    }

    #[test]
    fn validates_encoded_file_names() {
        assert_eq!(normalize_extension(" .FLAC "), Some("flac".to_string()));
        assert!(normalize_extension("wav/../x").is_none());
        assert!(is_safe_hash("abc-123_X"));
        assert!(is_safe_hash("abc-123_X.flac"));
        assert!(!is_safe_hash("../secret"));
        assert!(!is_safe_hash(".."));
        assert_eq!(
            encoded_file_name("abc-123_X.flac", "opus"),
            "abc-123_X.opus"
        );
    }

    #[test]
    fn resolves_source_extension_from_name_then_hash() {
        assert_eq!(
            source_extension("record.FLAC", "hash.wav"),
            Some("flac".to_string())
        );
        assert_eq!(source_extension("", "hash.aiff"), Some("aiff".to_string()));
        assert_eq!(source_extension("record", "hash"), None);
    }
    #[test]
    fn pure_rust_codecs_write_ogg_opus_and_adts_aac() {
        let root =
            std::env::temp_dir().join(format!("dantalian-audio-codec-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory");

        let source_path = root.join("source.wav");
        let opus_path = root.join("encoded.opus");
        let aac_path = root.join("encoded.aac");
        std::fs::write(&source_path, pcm_wav(48_000, 1, 4_800)).expect("source wav");
        let profile = AudioProfile {
            source_rate: 48_000,
            target_rate: 48_000,
            channels: 1,
        };

        encode_opus(&source_path, &opus_path, "wav", profile).expect("Opus encoding");
        encode_aac_with_rust(&source_path, &aac_path, "wav", profile).expect("AAC encoding");

        let opus = std::fs::read(&opus_path).expect("Opus output");
        let aac = std::fs::read(&aac_path).expect("AAC output");
        assert!(opus.starts_with(b"OggS"));
        assert!(aac.starts_with(&[0xff, 0xf1]));

        let _ = std::fs::remove_dir_all(root);
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
