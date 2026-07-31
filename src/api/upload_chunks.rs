use serde::Deserialize;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub(crate) const MAX_CHUNK_BYTES: usize = 90 * 1024 * 1024;

const MAX_UPLOAD_PARTS: usize = 1024;
const MAX_UPLOAD_ID_LENGTH: usize = 128;
const STALE_UPLOAD_AGE: Duration = Duration::from_secs(24 * 60 * 60);

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
    Partial {
        part: usize,
        total_parts: usize,
    },
    Complete {
        bytes: Vec<u8>,
        cleanup_dir: PathBuf,
    },
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

pub(crate) fn store_chunk(
    uploads_dir: &str,
    category: &str,
    info: ChunkInfo,
    bytes: &[u8],
) -> Result<StoreResult, ()> {
    if bytes.is_empty() || bytes.len() > MAX_CHUNK_BYTES {
        return Err(());
    }

    let upload_dir = Path::new(uploads_dir).join(category).join(&info.upload_id);
    fs::create_dir_all(&upload_dir).map_err(|_| ())?;

    let part_path = upload_dir.join(format!("part-{:06}.bin", info.part));
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
        if total_size > super::UPLOAD_ROUTE_LIMIT_BYTES {
            return Err(());
        }
    }

    let mut combined = Vec::with_capacity(total_size);
    for part in 0..info.total_parts {
        let path = upload_dir.join(format!("part-{:06}.bin", part));
        let mut file = fs::File::open(path).map_err(|_| ())?;
        file.read_to_end(&mut combined).map_err(|_| ())?;
    }

    Ok(StoreResult::Complete {
        bytes: combined,
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

        let partial = store_chunk(&uploads_dir.to_string_lossy(), "audio", first, b"first")
            .expect("first chunk should be stored");
        assert!(matches!(
            partial,
            StoreResult::Partial {
                part: 0,
                total_parts: 2
            }
        ));

        let complete = store_chunk(&uploads_dir.to_string_lossy(), "audio", second, b"second")
            .expect("last chunk should be reassembled");
        match complete {
            StoreResult::Complete { bytes, cleanup_dir } => {
                assert_eq!(bytes, b"firstsecond");
                fs::remove_dir_all(cleanup_dir).expect("test upload directory should be removed");
            }
            StoreResult::Partial { .. } => panic!("last chunk should complete the upload"),
        }

        assert!(!uploads_dir.join("audio").join("upload").exists());
        fs::remove_dir_all(&uploads_dir).expect("test uploads directory should be removed");
    }
}
