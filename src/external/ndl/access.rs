use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::sync::LazyLock;
use tokio::sync::{Mutex, MutexGuard};
use tokio::time::{Duration, sleep};

pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const REQUEST_INTERVAL: Duration = Duration::from_secs(2);
static GATE: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
static LOCK_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| std::env::temp_dir().join("dantalian-ndl-request.lock"));

pub(crate) struct RequestGuard {
    _file: File,
    _process_guard: MutexGuard<'static, ()>,
}

pub(crate) async fn acquire() -> Result<RequestGuard, String> {
    let process_guard = GATE.lock().await;
    let file = tokio::task::spawn_blocking(|| {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&*LOCK_PATH)?;
        FileExt::lock_exclusive(&file)?;
        Ok::<_, std::io::Error>(file)
    })
    .await
    .map_err(|error| format!("Catalog request lock task failed: {error}"))?
    .map_err(|error| format!("Catalog request lock failed: {error}"))?;
    let guard = RequestGuard {
        _file: file,
        _process_guard: process_guard,
    };
    // File close releases the OS lock on success, error, and future cancellation.
    // Waiting while holding both locks also provides a gap after a prior process exits.
    sleep(REQUEST_INTERVAL).await;
    Ok(guard)
}
