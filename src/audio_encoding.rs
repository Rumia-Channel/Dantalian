use crate::{audio_codec, db::Db};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::Notify;

pub const KEY_ENABLED: &str = "audio.data_saver.enabled";
pub const KEY_EXTENSIONS: &str = "audio.data_saver.extensions";
pub const DEFAULT_EXTENSIONS: &str = "wav,flac,aiff,alac";

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
    let source = fs::read(&source_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            "Original audio file not found".to_string()
        } else {
            format!("Original audio file could not be read: {}", error)
        }
    })?;

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

    if !variants.opus {
        match audio_codec::encode_opus(&source, &source_extension, file_hash)
            .and_then(|encoded| write_encoded_file(&opus_path, &encoded))
        {
            Ok(()) => variants.opus = true,
            Err(error) => tracing::warn!(file_hash, "Opus generation failed: {}", error),
        }
    }
    if !variants.aac {
        match audio_codec::encode_aac(&source, &source_extension)
            .and_then(|encoded| write_encoded_file(&aac_path, &encoded))
        {
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

pub fn normalize_extension(value: &str) -> Option<String> {
    audio_codec::normalize_extension(value)
}

fn write_encoded_file(target_path: &Path, encoded: &[u8]) -> Result<(), String> {
    let temporary_path = temporary_path(target_path);
    fs::write(&temporary_path, encoded).map_err(|error| error.to_string())?;
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
        AudioDataSaverConfig, encoded_file_name, ensure_encoded_variants, is_safe_hash,
        normalize_extension, source_extension,
    };
    use crate::{audio_codec::opus_sample_rate, db::Db};

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
    fn writes_native_encoded_variants_through_the_filesystem_sink() {
        let root =
            std::env::temp_dir().join(format!("dantalian-audio-encoding-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("audio directory");
        std::fs::write(root.join("source.wav"), pcm_wav(48_000, 1, 4_800)).expect("source audio");

        let variants = ensure_encoded_variants(&root.to_string_lossy(), "source.wav", "wav")
            .expect("encoded variants");

        assert!(variants.opus);
        assert!(variants.aac);
        assert!(root.join("encoded/opus/source.opus").is_file());
        assert!(root.join("encoded/aac/source.aac").is_file());
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
