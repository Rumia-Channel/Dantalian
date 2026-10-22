use axum_extra::extract::multipart::Field;
use serde::Deserialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub(crate) const MAX_CHUNK_BYTES: usize = 90 * 1024 * 1024;

const MAX_UPLOAD_PARTS: usize = 1024;
const MAX_UPLOAD_ID_LENGTH: usize = 128;
const STALE_UPLOAD_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// multipart のフィールドを上限付きで読み込む。
///
/// `field.bytes()` は上限判定より先に全体をメモリへ展開するため、巨大な
/// リクエストでプロセスが枯渇する。ここではチャンク単位で読み、上限を
/// 超えた時点で打ち切る。
pub(crate) async fn read_field_limited(
    field: &mut Field,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    let mut data = Vec::new();
    loop {
        let chunk = field
            .chunk()
            .await
            .map_err(|error| format!("invalid uploaded file: {error}"))?;
        let Some(chunk) = chunk else {
            break;
        };
        let new_len = data.len().saturating_add(chunk.len());
        if new_len > max_bytes {
            return Err(format!("file too large (max {} bytes)", max_bytes));
        }
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct ChunkQuery {
    pub upload_id: Option<String>,
    pub part: Option<usize>,
    pub total_parts: Option<usize>,
}

#[derive(Clone)]
pub(crate) struct ChunkInfo {
    pub upload_id: String,
    pub part: usize,
    pub total_parts: usize,
}

pub(crate) enum StoreResult {
    Partial { part: usize, total_parts: usize },
    Complete { path: PathBuf, cleanup_dir: PathBuf },
}

impl ChunkQuery {
    pub(crate) fn validate(&self) -> Result<Option<ChunkInfo>, ()> {
        let present = [
            self.upload_id.is_some(),
            self.part.is_some(),
            self.total_parts.is_some(),
        ];
        if !present.iter().any(|v| *v) {
            return Ok(None);
        }
        if !present.iter().all(|v| *v) {
            return Err(());
        }

        let upload_id = self.upload_id.as_ref().ok_or(())?;
        if upload_id.is_empty()
            || upload_id.len() > MAX_UPLOAD_ID_LENGTH
            || !upload_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(());
        }

        let part = self.part.ok_or(())?;
        let total_parts = self.total_parts.ok_or(())?;
        if total_parts == 0 || total_parts > MAX_UPLOAD_PARTS || part >= total_parts {
            return Err(());
        }

        Ok(Some(ChunkInfo {
            upload_id: upload_id.clone(),
            part,
            total_parts,
        }))
    }
}

/// `upload_dir` に保存済みのパート (`part-*.bin`) の合計サイズ。
/// ディレクトリがまだ無い場合は 0 を返す。
fn parts_size(upload_dir: &Path) -> Result<usize, ()> {
    let entries = match fs::read_dir(upload_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(_) => return Err(()),
    };
    let mut total = 0usize;
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with("part-") {
            continue;
        }
        let size = entry.metadata().map_err(|_| ())?.len();
        total = total
            .checked_add(usize::try_from(size).map_err(|_| ())?)
            .ok_or(())?;
    }
    Ok(total)
}

pub(crate) fn store_chunk(
    uploads_dir: &str,
    category: &str,
    info: ChunkInfo,
    bytes: &[u8],
    max_bytes: usize,
) -> Result<StoreResult, ()> {
    if bytes.is_empty() || bytes.len() > MAX_CHUNK_BYTES {
        return Err(());
    }

    let upload_dir = Path::new(uploads_dir).join(category).join(&info.upload_id);
    let part_path = upload_dir.join(format!("part-{:06}.bin", info.part));
    // 既に保存済みのパートと合算して上限を超えないことを書き込み前に確認する。
    // 同じパートの再送は置き換えなので、その分は加算から除く。
    let existing_total = parts_size(&upload_dir)?;
    let replaced_total = fs::metadata(&part_path)
        .ok()
        .and_then(|metadata| usize::try_from(metadata.len()).ok())
        .unwrap_or(0);
    let retained_total = existing_total.saturating_sub(replaced_total);
    let pending_total = retained_total.checked_add(bytes.len()).ok_or(())?;
    if pending_total > max_bytes {
        return Err(());
    }
    fs::create_dir_all(&upload_dir).map_err(|_| ())?;

    fs::write(&part_path, bytes).map_err(|_| ())?;

    if info.part + 1 != info.total_parts {
        return Ok(StoreResult::Partial {
            part: info.part,
            total_parts: info.total_parts,
        });
    }

    let mut total_size = 0usize;
    for part in 0..info.total_parts {
        let path = upload_dir.join(format!("part-{:06}.bin", part));
        let size = fs::metadata(path).map_err(|_| ())?.len();
        total_size = total_size
            .checked_add(usize::try_from(size).map_err(|_| ())?)
            .ok_or(())?;
        if total_size > max_bytes {
            return Err(());
        }
    }

    let combined_path = upload_dir.join("combined.bin");
    let mut combined = fs::File::create(&combined_path).map_err(|_| ())?;
    for part in 0..info.total_parts {
        let path = upload_dir.join(format!("part-{:06}.bin", part));
        let mut file = fs::File::open(path).map_err(|_| ())?;
        std::io::copy(&mut file, &mut combined).map_err(|_| ())?;
    }
    combined.flush().map_err(|_| ())?;

    Ok(StoreResult::Complete {
        path: combined_path,
        cleanup_dir: upload_dir,
    })
}

pub(crate) fn cleanup_stale_uploads(uploads_dir: &str) {
    let cutoff = SystemTime::now().checked_sub(STALE_UPLOAD_AGE);
    let Some(cutoff) = cutoff else { return };

    let Ok(categories) = fs::read_dir(uploads_dir) else {
        return;
    };
    for category in categories.flatten() {
        let Ok(entries) = fs::read_dir(category.path()) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(modified) = entry.metadata().and_then(|m| m.modified()) else {
                continue;
            };
            if modified < cutoff {
                let _ = fs::remove_dir_all(path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_uploads_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "dantalian-upload-chunks-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos()
        ))
    }

    #[test]
    fn validates_complete_chunk_metadata() {
        let query = ChunkQuery {
            upload_id: Some("upload_123".to_string()),
            part: Some(1),
            total_parts: Some(2),
        };

        let info = query.validate().expect("chunk metadata should be valid");
        assert_eq!(info.expect("chunk metadata should be present").part, 1);
    }

    #[test]
    fn rejects_partial_metadata_and_path_traversal() {
        let partial = ChunkQuery {
            upload_id: Some("upload".to_string()),
            part: Some(0),
            total_parts: None,
        };
        assert!(partial.validate().is_err());

        let traversal = ChunkQuery {
            upload_id: Some("../outside".to_string()),
            part: Some(0),
            total_parts: Some(1),
        };
        assert!(traversal.validate().is_err());
    }

    #[test]
    fn rejects_chunks_beyond_the_cumulative_limit() {
        let uploads_dir = temporary_uploads_dir();
        let first = ChunkInfo {
            upload_id: "limited".to_string(),
            part: 0,
            total_parts: 2,
        };
        store_chunk(&uploads_dir.to_string_lossy(), "audio", first, b"1234", 6)
            .expect("first chunk should fit");
        let second = ChunkInfo {
            upload_id: "limited".to_string(),
            part: 1,
            total_parts: 2,
        };
        assert!(
            store_chunk(&uploads_dir.to_string_lossy(), "audio", second, b"5678", 6,).is_err(),
            "chunks beyond the cumulative limit must be rejected"
        );
        fs::remove_dir_all(&uploads_dir).expect("test uploads directory should be removed");
    }

    #[test]
    fn reassembles_chunks_in_order() {
        let uploads_dir = temporary_uploads_dir();
        let first = ChunkInfo {
            upload_id: "upload".to_string(),
            part: 0,
            total_parts: 2,
        };
        let second = ChunkInfo {
            upload_id: "upload".to_string(),
            part: 1,
            total_parts: 2,
        };

        let partial = store_chunk(&uploads_dir.to_string_lossy(), "audio", first, b"first", 64)
            .expect("first chunk should be stored");
        assert!(matches!(
            partial,
            StoreResult::Partial {
                part: 0,
                total_parts: 2
            }
        ));

        let complete = store_chunk(
            &uploads_dir.to_string_lossy(),
            "audio",
            second,
            b"second",
            64,
        )
        .expect("last chunk should be reassembled");
        match complete {
            StoreResult::Complete { path, cleanup_dir } => {
                assert_eq!(fs::read(&path).expect("combined file"), b"firstsecond");
                fs::remove_dir_all(cleanup_dir).expect("test upload directory should be removed");
            }
            StoreResult::Partial { .. } => panic!("last chunk should complete the upload"),
        }

        assert!(!uploads_dir.join("audio").join("upload").exists());
        fs::remove_dir_all(&uploads_dir).expect("test uploads directory should be removed");
    }
}
