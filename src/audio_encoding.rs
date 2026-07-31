use crate::db::Db;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

pub const KEY_ENABLED: &str = "audio.data_saver.enabled";
pub const KEY_EXTENSIONS: &str = "audio.data_saver.extensions";
pub const DEFAULT_EXTENSIONS: &str = "wav,flac,aiff,alac";

const TARGET_BITRATE: u32 = 192_000;

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
        && value.len() <= 256
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
}

pub fn encoded_path(audio_dir: &str, file_hash: &str, format: &str) -> PathBuf {
    Path::new(audio_dir)
        .join("encoded")
        .join(format)
        .join(format!("{}.{}", file_hash, format))
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
    use shiguredo_opus::{Application, Encoder, EncoderConfig};
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let decoded = decode_audio(source_path, source_extension)?;
    let pcm = resample_audio(decoded, profile);
    let mut encoder = Encoder::new(EncoderConfig {
        bitrate: Some(TARGET_BITRATE),
        application: Some(Application::Audio),
        vbr: Some(true),
        ..EncoderConfig::new(profile.target_rate, profile.channels)
    })
    .map_err(|error| format!("Opus encoder creation failed: {}", error))?;
    let frame_samples = encoder.frame_samples() as usize;
    let frame_len = frame_samples * usize::from(profile.channels);
    let mut packets = Vec::new();
    for chunk in pcm.chunks(frame_len) {
        let mut frame = vec![0_i16; frame_len];
        frame[..chunk.len()].copy_from_slice(chunk);
        packets.push(
            encoder
                .encode(&frame)
                .map_err(|error| format!("Opus encode failed: {}", error))?,
        );
    }
    if packets.is_empty() {
        return Err("Opus encoder produced no packets".to_string());
    }

    let lookahead = encoder
        .get_lookahead()
        .map_err(|error| format!("Opus lookahead failed: {}", error))?;
    let lookahead_48k = u64::from(lookahead) * 48_000 / u64::from(profile.target_rate);
    let mut hasher = DefaultHasher::new();
    target_path.to_string_lossy().hash(&mut hasher);
    let serial = hasher.finish() as u32;
    let temporary_path = temporary_path(target_path);
    let file = File::create(&temporary_path).map_err(|error| error.to_string())?;
    let packet_count = packets.len();
    let mut writer = PacketWriter::new(BufWriter::new(file));
    writer
        .write_packet(
            opus_head(profile.source_rate, profile.channels, lookahead_48k as u16),
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
    _source_extension: &str,
    profile: AudioProfile,
) -> Result<(), String> {
    #[cfg(all(feature = "fdk-aac", target_os = "linux"))]
    {
        match encode_aac_with_fdk(source_path, target_path, _source_extension, profile) {
            Ok(()) => return Ok(()),
            Err(error) => tracing::warn!(
                "FDK AAC generation failed; using ffmpeg AAC fallback: {}",
                error
            ),
        }
    }

    // The native ffmpeg AAC encoder's quality mode is VBR and does not require libfdk-aac.
    encode_with_ffmpeg(
        source_path,
        target_path,
        profile,
        ["-c:a", "aac", "-q:a", "2", "-f", "adts"],
    )
}

fn ffmpeg_path() -> String {
    std::env::var("DANTALIAN_FFMPEG_PATH").unwrap_or_else(|_| "ffmpeg".to_string())
}

fn encode_with_ffmpeg<const N: usize>(
    source_path: &Path,
    target_path: &Path,
    profile: AudioProfile,
    codec_args: [&str; N],
) -> Result<(), String> {
    use std::process::{Command, Stdio};

    let temporary_path = temporary_path(target_path);
    let mut command = Command::new(ffmpeg_path());
    command
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(source_path)
        .args(["-map", "0:a:0", "-vn", "-sn", "-dn", "-ar"])
        .arg(profile.target_rate.to_string())
        .args(["-ac"])
        .arg(profile.channels.to_string())
        .args(codec_args)
        .arg(&temporary_path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let output = command
        .output()
        .map_err(|error| format!("ffmpeg could not be started: {}", error))?;
    if !output.status.success() {
        let _ = fs::remove_file(&temporary_path);
        return Err(format_command_error("ffmpeg", &output));
    }
    publish_temporary_file(&temporary_path, target_path)
}

#[cfg(all(feature = "fdk-aac", target_os = "linux"))]
fn encode_aac_with_fdk(
    source_path: &Path,
    target_path: &Path,
    source_extension: &str,
    profile: AudioProfile,
) -> Result<(), String> {
    use shiguredo_fdk_aac::{Encoder, EncoderConfig, FdkAacLibrary};

    let library_path = std::env::var("DANTALIAN_FDK_AAC_LIBRARY")
        .unwrap_or_else(|_| "libfdk-aac.so.2".to_string());
    let library = FdkAacLibrary::load(&library_path)
        .map_err(|error| format!("could not load {}: {}", library_path, error))?;
    let mut encoder = Encoder::new(
        library,
        EncoderConfig {
            sample_rate: profile.target_rate,
            channels: profile.channels,
            bitrate: Some(TARGET_BITRATE),
        },
    )
    .map_err(|error| format!("could not initialize FDK AAC: {}", error))?;
    let decoded = decode_audio(source_path, source_extension)?;
    let pcm = resample_audio(decoded, profile);
    encoder
        .encode(&pcm)
        .map_err(|error| format!("FDK AAC encode failed: {}", error))?;
    encoder
        .finish()
        .map_err(|error| format!("FDK AAC finish failed: {}", error))?;

    let mut frames = Vec::new();
    while let Some(frame) = encoder.next_frame() {
        frames.push(frame.data);
    }
    if frames.is_empty() {
        return Err("FDK AAC encoder produced no frames".to_string());
    }
    let asc = encoder.audio_specific_config().to_vec();
    let temporary_path = temporary_path(target_path);
    let output_file = File::create(&temporary_path).map_err(|error| error.to_string())?;
    let mut writer = BufWriter::new(output_file);
    for payload in frames {
        write_adts_frame(&mut writer, &asc, &payload)?;
    }
    writer.flush().map_err(|error| error.to_string())?;
    drop(writer);
    publish_temporary_file(&temporary_path, target_path)
}

#[cfg(all(feature = "fdk-aac", target_os = "linux"))]
fn write_adts_frame(writer: &mut impl Write, asc: &[u8], payload: &[u8]) -> Result<(), String> {
    if asc.len() < 2 || payload.len() + 7 > 0x1fff {
        return Err("Invalid FDK AAC frame parameters".to_string());
    }
    let object_type = ((asc[0] >> 3) & 0x1f).saturating_sub(1);
    let sample_rate_index = ((asc[0] & 0x07) << 1) | (asc[1] >> 7);
    let channel_config = (asc[1] >> 3) & 0x0f;
    if object_type > 3 || sample_rate_index > 12 || channel_config == 0 {
        return Err("Unsupported FDK AAC AudioSpecificConfig".to_string());
    }
    let frame_length = (payload.len() + 7) as u16;
    let header = [
        0xff,
        0xf1,
        (object_type << 6) | (sample_rate_index << 2) | (channel_config >> 2),
        ((channel_config & 0x03) << 6) | ((frame_length >> 11) as u8),
        (frame_length >> 3) as u8,
        (((frame_length & 0x07) as u8) << 5) | 0x1f,
        0xfc,
    ];
    writer
        .write_all(&header)
        .map_err(|error| error.to_string())?;
    writer.write_all(payload).map_err(|error| error.to_string())
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

fn format_command_error(command: &str, output: &std::process::Output) -> String {
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if detail.is_empty() {
        format!("{} exited with {}", command, output.status)
    } else {
        format!("{}: {}", command, detail)
    }
}

#[cfg(test)]
mod tests {
    use super::{is_safe_hash, normalize_extension, opus_sample_rate};

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
        assert!(!is_safe_hash("../secret"));
    }
}
