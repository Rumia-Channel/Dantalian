use std::collections::hash_map::DefaultHasher;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{BufWriter, Cursor, Write};
use std::path::Path;

use fdk_aac_rust::encoder::{
    ConfiguredPureRustEncoder, EncoderParameter, PureRustEncoderParameters,
};
use ogg::{PacketWriteEndInfo, PacketWriter};
use opus_rs::{Application, OpusEncoder};
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;

const DEFAULT_BITRATE_KBPS: u32 = 192;
const TARGET_BITRATE: u32 = DEFAULT_BITRATE_KBPS * 1_000;
const OPUS_LOOKAHEAD_48K: u64 = 312;
/// Keep the pure-Rust CBR encoder below the long-run reservoir edge case.
///
/// At high stereo CBR rates, the codec's automatic bandwidth can occasionally
/// request a frame larger than its available reservoir. Capping the
/// psychoacoustic bandwidth preserves the requested bitrate and keeps the
/// streaming path recoverable instead of leaving a partial AAC file.
fn aac_bandwidth_hz(bitrate_bps: u32, channels: u8) -> u32 {
    let per_channel = bitrate_bps / u32::from(channels.max(1));
    match per_channel {
        0..=24_000 => 2_000,
        24_001..=32_000 => 5_700,
        32_001..=40_000 => 8_800,
        40_001..=56_000 => 12_800,
        56_001..=64_000 => 15_000,
        _ => 15_000,
    }
}

fn bitrate_bps(bitrate_kbps: u32) -> Result<u32, String> {
    if !(8..=512).contains(&bitrate_kbps) {
        return Err("audio bitrate must be between 8 and 512 kbps".to_string());
    }
    bitrate_kbps
        .checked_mul(1_000)
        .ok_or_else(|| "audio bitrate overflows".to_string())
}
/// The codec libraries use large transient stack frames during construction and
/// encoding. Keep those frames off Tokio/test threads with a dedicated stack.
const CODEC_STACK_SIZE: usize = 8 * 1024 * 1024;

fn run_on_codec_stack<T, F>(job: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    std::thread::Builder::new()
        .name("dantalian-audio-codec".to_string())
        .stack_size(CODEC_STACK_SIZE)
        .spawn(job)
        .map_err(|error| format!("audio codec thread spawn failed: {error}"))?
        .join()
        .map_err(|_| "audio codec thread panicked".to_string())?
}

#[derive(Debug, Clone, Copy)]
struct AudioProfile {
    target_rate: u32,
    channels: u8,
}

#[derive(Debug)]
struct DecodedAudio {
    sample_rate: u32,
    channels: u8,
    samples: Vec<i16>,
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

pub fn opus_sample_rate(source_rate: u32) -> u32 {
    [8_000, 12_000, 16_000, 24_000, 48_000]
        .into_iter()
        .find(|target| source_rate <= *target)
        .unwrap_or(48_000)
}

pub fn encode_opus(
    source: &[u8],
    source_extension: &str,
    source_id: &str,
) -> Result<Vec<u8>, String> {
    let source_extension = normalize_extension(source_extension)
        .ok_or_else(|| "Invalid audio source extension".to_string())?;
    let decoded = decode_audio(source, &source_extension)?;
    let source_id = source_id.to_string();
    run_on_codec_stack(move || encode_opus_decoded(decoded, &source_id, Vec::new()))
}

pub fn encode_opus_file(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    source_extension: &str,
    source_id: &str,
) -> Result<(), String> {
    encode_opus_file_with_bitrate(
        input,
        output,
        source_extension,
        source_id,
        DEFAULT_BITRATE_KBPS,
    )
}

pub fn encode_opus_file_with_bitrate(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    source_extension: &str,
    source_id: &str,
    bitrate_kbps: u32,
) -> Result<(), String> {
    let bitrate_bps = bitrate_bps(bitrate_kbps)?;
    let source_extension = normalize_extension(source_extension)
        .ok_or_else(|| "Invalid audio source extension".to_string())?;
    let input = input.as_ref().to_path_buf();
    let output_path = output.as_ref().to_path_buf();
    let source_id = source_id.to_string();
    run_on_codec_stack(move || {
        let mut output = Some(BufWriter::new(
            File::create(&output_path)
                .map_err(|error| format!("Opus output create failed: {error}"))?,
        ));
        let mut encoder = None;
        stream_audio_file(&input, &source_extension, |profile, samples| {
            if encoder.is_none() {
                encoder = Some(OpusStreamEncoder::new(
                    profile,
                    &source_id,
                    bitrate_bps,
                    output.take().expect("Opus output initialized"),
                )?);
            }
            encoder
                .as_mut()
                .expect("Opus encoder initialized")
                .accept(samples)
        })?;
        encoder
            .ok_or_else(|| "audio contains no decodable samples".to_string())?
            .finish()?;
        Ok(())
    })
}

fn encode_opus_decoded<W: Write>(
    decoded: DecodedAudio,
    source_id: &str,
    output: W,
) -> Result<W, String> {
    let profile = profile_for(&decoded);
    let pcm = resample_audio(decoded, profile);
    let mut encoder = Box::new(
        OpusEncoder::new(
            profile.target_rate as i32,
            usize::from(profile.channels),
            Application::Audio,
        )
        .map_err(|error| format!("Opus encoder creation failed: {error}"))?,
    );
    encoder.bitrate_bps = TARGET_BITRATE as i32;
    encoder.use_cbr = false;

    let frame_samples = (profile.target_rate / 50) as usize;
    let frame_len = frame_samples * usize::from(profile.channels);
    let mut writer = PacketWriter::new(output);
    let mut hasher = DefaultHasher::new();
    source_id.hash(&mut hasher);
    let serial = hasher.finish() as u32;
    let pre_skip = (OPUS_LOOKAHEAD_48K * u64::from(profile.target_rate) / 48_000) as u16;
    writer
        .write_packet(
            opus_head(profile.target_rate, profile.channels, pre_skip),
            serial,
            PacketWriteEndInfo::EndPage,
            0,
        )
        .map_err(|error| format!("Ogg Opus header write failed: {error}"))?;
    writer
        .write_packet(opus_tags(), serial, PacketWriteEndInfo::EndPage, 0)
        .map_err(|error| format!("Ogg Opus tags write failed: {error}"))?;

    let pcm_frames = (pcm.len() / usize::from(profile.channels)) as u64;
    let final_granule_position = OPUS_LOOKAHEAD_48K
        + (pcm_frames * 48_000 + u64::from(profile.target_rate) - 1)
            / u64::from(profile.target_rate);
    let frame_samples_u64 = frame_samples as u64;
    let mut granule_position = 0;
    let mut chunks = pcm.chunks(frame_len).peekable();
    let mut wrote_packet = false;
    while let Some(chunk) = chunks.next() {
        let is_last = chunks.peek().is_none();
        let is_partial = chunk.len() < frame_len;
        let mut frame = vec![0.0_f32; frame_len];
        for (destination, sample) in frame.iter_mut().zip(chunk) {
            *destination = f32::from(*sample) / 32_768.0;
        }
        let mut packet = vec![0_u8; 4096];
        let packet_len = encoder
            .encode(&frame, frame_samples, &mut packet)
            .map_err(|error| format!("Opus encode failed: {error}"))?;
        packet.truncate(packet_len);
        let next_granule_position =
            granule_position + frame_samples_u64 * 48_000 / u64::from(profile.target_rate);
        writer
            .write_packet(
                packet,
                serial,
                if is_last && is_partial {
                    PacketWriteEndInfo::EndStream
                } else {
                    PacketWriteEndInfo::NormalPacket
                },
                if is_last && is_partial {
                    final_granule_position
                } else {
                    next_granule_position
                },
            )
            .map_err(|error| format!("Ogg Opus packet write failed: {error}"))?;
        wrote_packet = true;
        granule_position = next_granule_position;

        if is_last && !is_partial {
            let padding = vec![0.0_f32; frame_len];
            let mut packet = vec![0_u8; 4096];
            let packet_len = encoder
                .encode(&padding, frame_samples, &mut packet)
                .map_err(|error| format!("Opus padding encode failed: {error}"))?;
            packet.truncate(packet_len);
            writer
                .write_packet(
                    packet,
                    serial,
                    PacketWriteEndInfo::EndStream,
                    final_granule_position,
                )
                .map_err(|error| format!("Ogg Opus final packet write failed: {error}"))?;
        }
    }
    if !wrote_packet {
        return Err("Opus encoder produced no packets".to_string());
    }
    Ok(writer.into_inner())
}

pub fn encode_aac(source: &[u8], source_extension: &str) -> Result<Vec<u8>, String> {
    let source_extension = normalize_extension(source_extension)
        .ok_or_else(|| "Invalid audio source extension".to_string())?;
    let decoded = decode_audio(source, &source_extension)?;
    run_on_codec_stack(move || encode_aac_decoded(decoded, Vec::new()))
}

pub fn encode_aac_file(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    source_extension: &str,
) -> Result<(), String> {
    encode_aac_file_with_bitrate(input, output, source_extension, DEFAULT_BITRATE_KBPS)
}

pub fn encode_aac_file_with_bitrate(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    source_extension: &str,
    bitrate_kbps: u32,
) -> Result<(), String> {
    let bitrate_bps = bitrate_bps(bitrate_kbps)?;
    let source_extension = normalize_extension(source_extension)
        .ok_or_else(|| "Invalid audio source extension".to_string())?;
    let input = input.as_ref().to_path_buf();
    let output_path = output.as_ref().to_path_buf();
    run_on_codec_stack(move || {
        let mut output = Some(BufWriter::new(
            File::create(&output_path)
                .map_err(|error| format!("AAC output create failed: {error}"))?,
        ));
        let mut encoder = None;
        stream_audio_file(&input, &source_extension, |profile, samples| {
            if encoder.is_none() {
                encoder = Some(AacStreamEncoder::new(
                    profile,
                    bitrate_bps,
                    output.take().expect("AAC output initialized"),
                )?);
            }
            encoder
                .as_mut()
                .expect("AAC encoder initialized")
                .accept(samples)
        })?;
        encoder
            .ok_or_else(|| "audio contains no decodable samples".to_string())?
            .finish()?;
        Ok(())
    })
}

fn encode_aac_decoded<W: Write>(decoded: DecodedAudio, output: W) -> Result<W, String> {
    let profile = profile_for(&decoded);
    let pcm = resample_audio(decoded, profile);
    let mut parameters = PureRustEncoderParameters::new(2);
    parameters
        .set_parameter(EncoderParameter::AudioObjectType, 2)
        .map_err(|error| format!("invalid AAC audio object type: {error:?}"))?;
    parameters
        .set_parameter(EncoderParameter::ChannelMode, u32::from(profile.channels))
        .map_err(|error| format!("invalid AAC channel mode: {error:?}"))?;
    parameters
        .set_parameter(EncoderParameter::SampleRate, profile.target_rate)
        .map_err(|error| format!("invalid AAC sample rate: {error:?}"))?;
    parameters
        .set_parameter(EncoderParameter::Bitrate, TARGET_BITRATE)
        .map_err(|error| format!("invalid AAC bitrate: {error:?}"))?;
    parameters
        .set_parameter(
            EncoderParameter::Bandwidth,
            aac_bandwidth_hz(TARGET_BITRATE, profile.channels),
        )
        .map_err(|error| format!("invalid AAC bandwidth: {error:?}"))?;
    parameters
        .set_parameter(EncoderParameter::TransportMux, 2)
        .map_err(|error| format!("invalid AAC transport: {error:?}"))?;
    let mut encoder = Box::new(
        ConfiguredPureRustEncoder::from_parameters(&parameters)
            .map_err(|error| format!("could not initialize AAC encoder: {error:?}"))?,
    );
    let frame_len = encoder.input_samples_per_channel() * usize::from(profile.channels);
    let mut output = output;
    let mut encoded_frames = 0;
    for chunk in pcm.chunks(frame_len) {
        let mut frame = vec![0.0_f32; frame_len];
        for (destination, sample) in frame.iter_mut().zip(chunk) {
            *destination = f32::from(*sample) / 32_768.0;
        }
        let encoded = encoder
            .encode_transport_f32(&frame)
            .map_err(|error| format!("AAC encode failed: {error:?}"))?;
        if !encoded.is_empty() {
            output
                .write_all(&encoded)
                .map_err(|error| error.to_string())?;
            encoded_frames += 1;
        }
    }
    if encoded_frames == 0 {
        return Err("AAC encoder produced no frames".to_string());
    }
    Ok(output)
}

fn profile_for(decoded: &DecodedAudio) -> AudioProfile {
    AudioProfile {
        target_rate: opus_sample_rate(decoded.sample_rate),
        channels: decoded.channels,
    }
}

fn decode_audio(source: &[u8], source_extension: &str) -> Result<DecodedAudio, String> {
    decode_audio_source(Cursor::new(source), source_extension)
}

struct StreamingResampler {
    source_rate: u32,
    target_rate: u32,
    channels: usize,
    source: Vec<i16>,
    base_frame: u64,
    next_source_position: f64,
}

impl StreamingResampler {
    fn new(source_rate: u32, target_rate: u32, channels: usize) -> Self {
        Self {
            source_rate,
            target_rate,
            channels,
            source: Vec::new(),
            base_frame: 0,
            next_source_position: 0.0,
        }
    }

    fn push(&mut self, samples: &[i16], end_of_stream: bool) -> Vec<i16> {
        self.source.extend_from_slice(samples);
        let available_frames = self.base_frame + (self.source.len() / self.channels) as u64;
        let mut output = Vec::new();
        loop {
            let source_frame = self.next_source_position.floor() as u64;
            if source_frame >= available_frames
                || (!end_of_stream && source_frame + 1 >= available_frames)
            {
                break;
            }
            let first = ((source_frame - self.base_frame) as usize) * self.channels;
            let second_frame = (source_frame + 1).min(available_frames - 1);
            let second = ((second_frame - self.base_frame) as usize) * self.channels;
            let fraction = self.next_source_position.fract();
            for channel in 0..self.channels {
                let first_sample = f64::from(self.source[first + channel]);
                let second_sample = f64::from(self.source[second + channel]);
                output.push(
                    (first_sample + (second_sample - first_sample) * fraction)
                        .round()
                        .clamp(-32768.0, 32767.0) as i16,
                );
            }
            self.next_source_position += f64::from(self.source_rate) / f64::from(self.target_rate);
        }
        let keep_from = (self.next_source_position.floor() as u64).saturating_sub(1);
        if keep_from > self.base_frame {
            let remove_frames = (keep_from - self.base_frame) as usize;
            self.source.drain(..remove_frames * self.channels);
            self.base_frame = keep_from;
        }
        output
    }
}

fn stream_audio_file<F>(
    input: impl AsRef<Path>,
    source_extension: &str,
    mut consume: F,
) -> Result<AudioProfile, String>
where
    F: FnMut(AudioProfile, &[i16]) -> Result<(), String>,
{
    let source = File::open(input).map_err(|error| format!("audio input open failed: {error}"))?;
    let mut hint = Hint::new();
    hint.with_extension(source_extension);
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            MediaSourceStream::new(Box::new(source), Default::default()),
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|error| format!("audio format probe failed: {error}"))?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| "audio track not found".to_string())?;
    let track_id = track.id;
    let codec_params = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
        .ok_or_else(|| "audio codec parameters not found".to_string())?;
    let source_rate = codec_params.sample_rate.unwrap_or(48_000);
    let source_channels = codec_params
        .channels
        .as_ref()
        .map(|value| value.count().clamp(1, 2))
        .unwrap_or(2);
    let profile = AudioProfile {
        target_rate: opus_sample_rate(source_rate),
        channels: source_channels as u8,
    };
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(codec_params, &AudioDecoderOptions::default())
        .map_err(|error| format!("audio decoder creation failed: {error}"))?;
    let mut resampler = StreamingResampler::new(source_rate, profile.target_rate, source_channels);
    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) | Err(SymphoniaError::ResetRequired) => break,
            Err(error) => return Err(format!("audio packet read failed: {error}")),
        };
        if packet.track_id != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) | Err(SymphoniaError::IoError(_)) => continue,
            Err(error) => return Err(format!("audio decode failed: {error}")),
        };
        let decoded_channels = decoded.spec().channels().count();
        let mut packet_samples = Vec::new();
        decoded.copy_to_vec_interleaved::<i16>(&mut packet_samples);
        let normalized = normalize_channels(&packet_samples, decoded_channels, source_channels);
        let resampled = resampler.push(&normalized, false);
        if !resampled.is_empty() {
            consume(profile, &resampled)?;
        }
    }
    let tail = resampler.push(&[], true);
    if !tail.is_empty() {
        consume(profile, &tail)?;
    }
    Ok(profile)
}

struct OpusStreamEncoder<W: Write> {
    writer: PacketWriter<'static, W>,
    encoder: Box<OpusEncoder>,
    serial: u32,
    target_rate: u32,
    channels: usize,
    frame_samples: usize,
    frame_len: usize,
    pending_pcm: Vec<i16>,
    pending_packet: Option<(Vec<u8>, u64)>,
    encoded_pcm_frames: u64,
    total_pcm_frames: u64,
}

impl<W: Write> OpusStreamEncoder<W> {
    fn new(
        profile: AudioProfile,
        source_id: &str,
        bitrate_bps: u32,
        output: W,
    ) -> Result<Self, String> {
        let mut encoder = Box::new(
            OpusEncoder::new(
                profile.target_rate as i32,
                usize::from(profile.channels),
                Application::Audio,
            )
            .map_err(|error| format!("Opus encoder creation failed: {error}"))?,
        );
        encoder.bitrate_bps =
            i32::try_from(bitrate_bps).map_err(|_| "Opus bitrate is too large".to_string())?;
        encoder.use_cbr = false;
        let mut hasher = DefaultHasher::new();
        source_id.hash(&mut hasher);
        let serial = hasher.finish() as u32;
        let pre_skip = (OPUS_LOOKAHEAD_48K * u64::from(profile.target_rate) / 48_000) as u16;
        let mut writer = PacketWriter::new(output);
        writer
            .write_packet(
                opus_head(profile.target_rate, profile.channels, pre_skip),
                serial,
                PacketWriteEndInfo::EndPage,
                0,
            )
            .map_err(|error| format!("Ogg Opus header write failed: {error}"))?;
        writer
            .write_packet(opus_tags(), serial, PacketWriteEndInfo::EndPage, 0)
            .map_err(|error| format!("Ogg Opus tags write failed: {error}"))?;
        let frame_samples = (profile.target_rate / 50) as usize;
        Ok(Self {
            writer,
            encoder,
            serial,
            target_rate: profile.target_rate,
            channels: usize::from(profile.channels),
            frame_samples,
            frame_len: frame_samples * usize::from(profile.channels),
            pending_pcm: Vec::new(),
            pending_packet: None,
            encoded_pcm_frames: 0,
            total_pcm_frames: 0,
        })
    }

    fn accept(&mut self, samples: &[i16]) -> Result<(), String> {
        self.total_pcm_frames += (samples.len() / self.channels) as u64;
        self.pending_pcm.extend_from_slice(samples);
        let complete_len = self.pending_pcm.len() / self.frame_len * self.frame_len;
        if complete_len == 0 {
            return Ok(());
        }
        let remainder = self.pending_pcm.split_off(complete_len);
        let complete = std::mem::replace(&mut self.pending_pcm, remainder);
        for frame in complete.chunks_exact(self.frame_len) {
            self.encode_frame(frame)?;
        }
        Ok(())
    }

    fn encode_frame(&mut self, samples: &[i16]) -> Result<(), String> {
        let mut frame = vec![0.0_f32; self.frame_len];
        for (destination, sample) in frame.iter_mut().zip(samples) {
            *destination = f32::from(*sample) / 32_768.0;
        }
        let mut packet = vec![0_u8; 4096];
        let packet_len = self
            .encoder
            .encode(&frame, self.frame_samples, &mut packet)
            .map_err(|error| format!("Opus encode failed: {error}"))?;
        packet.truncate(packet_len);
        self.encoded_pcm_frames += self.frame_samples as u64;
        let granule_position = (self.encoded_pcm_frames * 48_000 + u64::from(self.target_rate) - 1)
            / u64::from(self.target_rate);
        if let Some((previous, previous_granule)) = self.pending_packet.take() {
            self.writer
                .write_packet(
                    previous,
                    self.serial,
                    PacketWriteEndInfo::NormalPacket,
                    previous_granule,
                )
                .map_err(|error| format!("Ogg Opus packet write failed: {error}"))?;
        }
        self.pending_packet = Some((packet, granule_position));
        Ok(())
    }

    fn finish(mut self) -> Result<(), String> {
        if !self.pending_pcm.is_empty() {
            let mut frame = vec![0_i16; self.frame_len];
            frame[..self.pending_pcm.len()].copy_from_slice(&self.pending_pcm);
            self.encode_frame(&frame)?;
        } else {
            self.encode_frame(&vec![0_i16; self.frame_len])?;
        }
        let final_granule_position = OPUS_LOOKAHEAD_48K
            + (self.total_pcm_frames * 48_000 + u64::from(self.target_rate) - 1)
                / u64::from(self.target_rate);
        let Some((packet, _)) = self.pending_packet.take() else {
            return Err("Opus encoder produced no packets".to_string());
        };
        self.writer
            .write_packet(
                packet,
                self.serial,
                PacketWriteEndInfo::EndStream,
                final_granule_position,
            )
            .map_err(|error| format!("Ogg Opus packet write failed: {error}"))?;
        self.writer
            .into_inner()
            .flush()
            .map_err(|error| format!("Ogg Opus output flush failed: {error}"))?;
        Ok(())
    }
}

struct AacStreamEncoder<W: Write> {
    output: W,
    encoder: Box<ConfiguredPureRustEncoder>,
    frame_len: usize,
    pending_pcm: Vec<i16>,
    encoded_frames: u32,
}
impl<W: Write> AacStreamEncoder<W> {
    fn new(profile: AudioProfile, bitrate_bps: u32, output: W) -> Result<Self, String> {
        let mut parameters = PureRustEncoderParameters::new(2);
        parameters
            .set_parameter(EncoderParameter::AudioObjectType, 2)
            .map_err(|error| format!("invalid AAC audio object type: {error:?}"))?;
        parameters
            .set_parameter(EncoderParameter::ChannelMode, u32::from(profile.channels))
            .map_err(|error| format!("invalid AAC channel mode: {error:?}"))?;
        parameters
            .set_parameter(EncoderParameter::SampleRate, profile.target_rate)
            .map_err(|error| format!("invalid AAC sample rate: {error:?}"))?;
        parameters
            .set_parameter(EncoderParameter::Bitrate, bitrate_bps)
            .map_err(|error| format!("invalid AAC bitrate: {error:?}"))?;
        parameters
            .set_parameter(
                EncoderParameter::Bandwidth,
                aac_bandwidth_hz(bitrate_bps, profile.channels),
            )
            .map_err(|error| format!("invalid AAC bandwidth: {error:?}"))?;
        parameters
            .set_parameter(EncoderParameter::TransportMux, 2)
            .map_err(|error| format!("invalid AAC transport: {error:?}"))?;
        let encoder = Box::new(
            ConfiguredPureRustEncoder::from_parameters(&parameters)
                .map_err(|error| format!("could not initialize AAC encoder: {error:?}"))?,
        );
        let channels = usize::from(profile.channels);
        let frame_len = encoder.input_samples_per_channel() * channels;
        Ok(Self {
            output,
            encoder,
            frame_len,
            pending_pcm: Vec::new(),
            encoded_frames: 0,
        })
    }

    fn accept(&mut self, samples: &[i16]) -> Result<(), String> {
        self.pending_pcm.extend_from_slice(samples);
        let complete_len = self.pending_pcm.len() / self.frame_len * self.frame_len;
        if complete_len == 0 {
            return Ok(());
        }
        let remainder = self.pending_pcm.split_off(complete_len);
        let complete = std::mem::replace(&mut self.pending_pcm, remainder);
        for frame in complete.chunks_exact(self.frame_len) {
            self.encode_frame(frame)?;
        }
        Ok(())
    }

    fn encode_frame(&mut self, samples: &[i16]) -> Result<(), String> {
        let mut frame = vec![0.0_f32; self.frame_len];
        for (destination, sample) in frame.iter_mut().zip(samples) {
            *destination = f32::from(*sample) / 32_768.0;
        }
        let encoded = self
            .encoder
            .encode_transport_f32(&frame)
            .map_err(|error| format!("AAC encode failed: {error:?}"))?;
        if !encoded.is_empty() {
            self.output
                .write_all(&encoded)
                .map_err(|error| error.to_string())?;
            self.encoded_frames += 1;
        }
        Ok(())
    }

    fn finish(mut self) -> Result<(), String> {
        if !self.pending_pcm.is_empty() {
            let mut frame = vec![0_i16; self.frame_len];
            frame[..self.pending_pcm.len()].copy_from_slice(&self.pending_pcm);
            self.encode_frame(&frame)?;
        }
        if self.encoded_frames == 0 {
            return Err("AAC encoder produced no frames".to_string());
        }
        self.output
            .flush()
            .map_err(|error| format!("AAC output flush failed: {error}"))?;
        Ok(())
    }
}

fn decode_audio_source<S: MediaSource>(
    source: S,
    source_extension: &str,
) -> Result<DecodedAudio, String> {
    let mut hint = Hint::new();
    hint.with_extension(source_extension);
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            MediaSourceStream::new(Box::new(source), Default::default()),
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|error| format!("audio format probe failed: {error}"))?;
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
    let mut channels = codec_params
        .channels
        .as_ref()
        .map(|value| value.count().clamp(1, 2))
        .unwrap_or(2);
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
                channels = decoded_channels.clamp(1, 2);
                let mut packet_samples = Vec::new();
                decoded.copy_to_vec_interleaved::<i16>(&mut packet_samples);
                samples.extend(normalize_channels(
                    &packet_samples,
                    decoded_channels,
                    channels,
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
        channels: channels as u8,
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use ogg::PacketReader;
    use opus_rs::OpusDecoder;

    use super::{
        aac_bandwidth_hz, bitrate_bps, encode_aac, encode_aac_file, encode_aac_file_with_bitrate,
        encode_opus, encode_opus_file, encode_opus_file_with_bitrate, opus_sample_rate,
    };
    #[test]
    fn chooses_the_smallest_supported_opus_rate_not_below_source() {
        assert_eq!(opus_sample_rate(22_050), 24_000);
        assert_eq!(opus_sample_rate(44_100), 48_000);
        assert_eq!(opus_sample_rate(96_000), 48_000);
    }
    #[test]
    fn keeps_bitrate_parameters_in_explicit_units() {
        assert_eq!(bitrate_bps(8), Ok(8_000));
        assert_eq!(bitrate_bps(512), Ok(512_000));
        assert!(bitrate_bps(7).is_err());
        assert!(bitrate_bps(513).is_err());
        assert_eq!(aac_bandwidth_hz(192_000, 2), 15_000);
    }

    #[test]
    fn encodes_audio_bytes_without_a_filesystem() {
        let source = pcm_wav(48_000, 1, 4_800);
        let opus = encode_opus(&source, "wav", "source-hash").expect("Opus encoding");
        let aac = encode_aac(&source, "wav").expect("AAC encoding");
        assert!(opus.starts_with(b"OggS"));
        assert!(aac.starts_with(&[0xff, 0xf1]));
    }

    #[test]
    fn writes_ogg_opus_that_decodes_as_audio_packets() {
        let source = pcm_wav(48_000, 1, 4_800);
        let opus = encode_opus(&source, "wav", "decode-check").expect("Opus encoding");
        let mut reader = PacketReader::new(Cursor::new(opus));

        let head = reader
            .read_packet()
            .expect("OpusHead packet")
            .expect("OpusHead packet exists");
        assert_eq!(&head.data[..8], b"OpusHead");
        let channels = usize::from(head.data[9]);
        let sample_rate = i32::try_from(u32::from_le_bytes(head.data[12..16].try_into().unwrap()))
            .expect("sample rate fits i32");
        let mut decoder = OpusDecoder::new(sample_rate, channels).expect("Opus decoder");

        let tags = reader
            .read_packet()
            .expect("OpusTags packet")
            .expect("OpusTags packet exists");
        assert_eq!(&tags.data[..8], b"OpusTags");

        let mut packet_count = 0;
        let mut decoded_samples = 0;
        while let Some(packet) = reader.read_packet().expect("audio packet") {
            let mut output = vec![0.0_f32; 5_760 * channels];
            let samples = decoder
                .decode(&packet.data, 5_760, &mut output)
                .expect("Opus audio packet decodes");
            assert!(samples > 0);
            packet_count += 1;
            decoded_samples += samples;
        }

        assert_eq!(packet_count, 6);
        assert_eq!(decoded_samples, 6 * 960);
    }

    #[test]
    fn encodes_audio_file_to_a_streaming_output() {
        let root =
            std::env::temp_dir().join(format!("dantalian-audio-codec-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("create test directory");
        let input = root.join("input.wav");
        let opus_output = root.join("output.ogg");
        let aac_output = root.join("output.aac");
        std::fs::write(&input, pcm_wav(44_100, 2, 4_410)).expect("write input");

        encode_opus_file(&input, &opus_output, "wav", "file-source").expect("Opus file encoding");
        encode_aac_file(&input, &aac_output, "wav").expect("AAC file encoding");
        let opus = std::fs::read(&opus_output).expect("read Opus output");
        let aac = std::fs::read(&aac_output).expect("read AAC output");
        assert!(opus.starts_with(b"OggS"));
        assert!(aac.starts_with(&[0xff, 0xf1]));
        let mut reader = PacketReader::new(Cursor::new(opus));
        reader
            .read_packet()
            .expect("OpusHead packet")
            .expect("OpusHead packet exists");
        reader
            .read_packet()
            .expect("OpusTags packet")
            .expect("OpusTags packet exists");
        let mut packet_count = 0;
        while let Some(_) = reader.read_packet().expect("audio packet") {
            packet_count += 1;
        }
        assert_eq!(packet_count, 6);

        std::fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn applies_requested_bitrate_to_streaming_encoders() {
        let root = std::env::temp_dir().join(format!(
            "dantalian-audio-codec-bitrate-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create bitrate test directory");
        let input = root.join("input.wav");
        let opus_low = root.join("opus-low.ogg");
        let opus_high = root.join("opus-high.ogg");
        let aac_low = root.join("aac-low.aac");
        let aac_high = root.join("aac-high.aac");
        let mut input_bytes = pcm_wav(44_100, 2, 44_100);
        for (index, byte) in input_bytes[44..].iter_mut().enumerate() {
            *byte = ((index * 37) % 251) as u8;
        }
        std::fs::write(&input, input_bytes).expect("write bitrate input");

        encode_opus_file_with_bitrate(&input, &opus_low, "wav", "low", 256)
            .expect("low bitrate Opus encoding");
        encode_opus_file_with_bitrate(&input, &opus_high, "wav", "high", 512)
            .expect("high bitrate Opus encoding");
        encode_aac_file_with_bitrate(&input, &aac_low, "wav", 256)
            .expect("low bitrate AAC encoding");
        encode_aac_file_with_bitrate(&input, &aac_high, "wav", 512)
            .expect("high bitrate AAC encoding");

        assert_ne!(
            std::fs::metadata(&opus_low)
                .expect("low Opus metadata")
                .len(),
            std::fs::metadata(&opus_high)
                .expect("high Opus metadata")
                .len()
        );
        assert!(std::fs::metadata(&aac_low).expect("low AAC metadata").len() > 0);
        assert!(
            std::fs::metadata(&aac_high)
                .expect("high AAC metadata")
                .len()
                > 0
        );

        std::fs::remove_dir_all(root).expect("remove bitrate test directory");
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
