// プレイヤーページの初期化・UI レンダリング・コントロール配線。
// 再生そのものは PlayerEngine (engine.js) が担当する。

const engine = new PlayerEngine();

const els = {
    coverImg: document.getElementById("cover-img"),
    coverFallback: document.getElementById("cover-fallback"),
    cover: document.getElementById("cover"),
    ambientA: document.getElementById("ambient-a"),
    ambientB: document.getElementById("ambient-b"),
    trackTitle: document.getElementById("track-title"),
    albumTitle: document.getElementById("album-title"),
    artistName: document.getElementById("artist-name"),
    progressBar: document.getElementById("progress-bar"),
    progressFill: document.getElementById("progress-fill"),
    progressThumb: document.getElementById("progress-thumb"),
    timeCurrent: document.getElementById("time-current"),
    timeTotal: document.getElementById("time-total"),
    btnPlay: document.getElementById("btn-play"),
    playIcon: document.getElementById("play-icon"),
    btnPrev: document.getElementById("btn-prev"),
    btnNext: document.getElementById("btn-next"),
    btnPrevAlbum: document.getElementById("btn-prev-album"),
    btnNextAlbum: document.getElementById("btn-next-album"),
    btnClose: document.getElementById("btn-close"),
    btnMute: document.getElementById("btn-mute"),
    volIcon: document.getElementById("vol-icon"),
    volume: document.getElementById("volume"),
    tracklist: document.getElementById("tracklist"),
    tracklistCount: document.getElementById("tracklist-count"),
    tracklistToggle: document.getElementById("tracklist-toggle"),
    statusDot: document.getElementById("status-dot"),
    statusLabel: document.getElementById("status-label"),
};

let allAlbums = [];      // 再生可能トラックを持つ CD の一覧
let currentCd = null;    // 現在再生中の CD
let ambientFlip = false; // クロスフェード用交互レイヤー

function getCdIdFromUrl() {
    const p = new URLSearchParams(location.search);
    const v = p.get("cd");
    return v ? parseInt(v, 10) : null;
}

function formatTime(sec) {
    if (!isFinite(sec) || sec < 0) sec = 0;
    const m = Math.floor(sec / 60);
    const s = Math.floor(sec % 60);
    return `${m}:${String(s).padStart(2, "0")}`;
}

function coverUrl(cd) {
    return cd && cd.cover_url ? `/images/${cd.cover_url}` : null;
}

// ---------- アンビエント背景 ----------
function setAmbient(url) {
    const show = ambientFlip ? els.ambientA : els.ambientB;
    const hide = ambientFlip ? els.ambientB : els.ambientA;
    if (url) {
        show.style.backgroundImage = `url("${url}")`;
        show.classList.add("active");
        hide.classList.remove("active");
        ambientFlip = !ambientFlip;
    } else {
        els.ambientA.classList.remove("active");
        els.ambientB.classList.remove("active");
    }
}

// ---------- カバー ----------
function setCover(cd) {
    const url = coverUrl(cd);
    if (url) {
        els.coverImg.src = url;
        els.coverImg.alt = cd.title || "";
        els.coverImg.style.display = "block";
        els.coverFallback.style.display = "none";
    } else {
        els.coverImg.removeAttribute("src");
        els.coverImg.style.display = "none";
        els.coverFallback.style.display = "flex";
    }
    setAmbient(url);
}

// ---------- トラックリスト ----------
function renderTracklist(cd) {
    const tracks = engine.allTracks || [];
    const multiDisc = (cd.disc_count || 1) > 1 ||
        tracks.some((t) => (t.disc_number || 1) > 1);

    let html = "";
    let lastDisc = null;
    for (const t of tracks) {
        const disc = t.disc_number || 1;
        if (multiDisc && disc !== lastDisc) {
            html += `<li class="player-tracklist-disc">DISC ${disc}</li>`;
            lastDisc = disc;
        }
        const playable = !!t.file_hash;
        const num = String(t.track_number).padStart(2, "0");
        html += `
        <li class="player-track${playable ? "" : " player-track--disabled"}"
            data-track-id="${t.id}" ${playable ? `onclick="jumpToTrack(${t.id})"` : ""}>
            <span class="player-track-num">
                <span class="player-track-num-text">${num}</span>
                <span class="player-eq" aria-hidden="true"><i></i><i></i><i></i></span>
            </span>
            <span class="player-track-title-cell">${escapeHtml(t.title)}</span>
            <span class="player-track-dur">${t.duration ? escapeHtml(t.duration) : (playable ? "" : "—")}</span>
        </li>`;
    }
    els.tracklist.innerHTML = html || '<li class="player-track-empty">トラックがありません</li>';

    const playableCount = tracks.filter((t) => t.file_hash).length;
    els.tracklistCount.textContent = `${playableCount} 曲`;
}

function highlightCurrentTrack() {
    const cur = engine.current();
    els.tracklist.querySelectorAll(".player-track").forEach((li) => {
        const id = parseInt(li.dataset.trackId, 10);
        li.classList.toggle("player-track--current", !!cur && cur.id === id);
    });
}

function jumpToTrack(trackId) {
    engine.playTrackById(trackId);
}

// ---------- メタ情報 ----------
function updateNowPlaying() {
    const t = engine.current();
    els.trackTitle.textContent = t ? t.title : "再生できるトラックがありません";
    els.albumTitle.textContent = currentCd ? currentCd.title : "—";
    els.artistName.textContent = currentCd ? (currentCd.artist || "") : "";

    document.title = t && currentCd
        ? `${t.title} - ${currentCd.title} | Dantalian`
        : "Dantalian - プレイヤー";

    highlightCurrentTrack();
    updateMediaSession();
}

function updateMediaSession() {
    if (!("mediaSession" in navigator) || !currentCd) return;
    const t = engine.current();
    try {
        navigator.mediaSession.metadata = new MediaMetadata({
            title: t ? t.title : currentCd.title,
            artist: currentCd.artist || "",
            album: currentCd.title,
            artwork: coverUrl(currentCd) ? [{ src: coverUrl(currentCd), sizes: "512x512", type: "image/jpeg" }] : [],
        });
    } catch {}
}

// ---------- 進捗 ----------
function updateProgress(pos) {
    const dur = engine.getDuration();
    const frac = dur > 0 ? Math.min(pos / dur, 1) : 0;
    const pct = (frac * 100).toFixed(3) + "%";
    els.progressFill.style.width = pct;
    els.progressThumb.style.left = pct;
    els.timeCurrent.textContent = formatTime(pos);
}

function updateDuration(dur) {
    els.timeTotal.textContent = formatTime(dur);
}

// ---------- アルバム移動 ----------
function buildAlbumList(cds) {
    return cds
        .filter((c) => (c.tracks || []).some((t) => t.file_hash))
        .sort((a, b) => (a.title || "").localeCompare(b.title || "", "ja"));
}

function currentAlbumIndex() {
    if (!currentCd) return -1;
    return allAlbums.findIndex((c) => c.id === currentCd.id);
}

function goToAlbum(delta) {
    if (allAlbums.length === 0) return;
    let idx = currentAlbumIndex();
    if (idx < 0) idx = delta > 0 ? -1 : 0;
    const next = (idx + delta + allAlbums.length) % allAlbums.length;
    loadAlbum(allAlbums[next].id, null, true);
}

// ---------- アルバム読み込み ----------
async function loadAlbum(cdId, startTrackId, autoplay) {
    const cd = allAlbums.find((c) => c.id === cdId);
    if (!cd) return;
    currentCd = cd;
    setCover(cd);
    renderTracklist(cd);
    engine.loadTracks(cd.tracks || [], startTrackId);
    updateNowPlaying();
    updateProgress(0);
    updateDuration(0);
    if (autoplay) engine.play();

    // URL を書き換えてリロードしても同じアルバムが開くようにする
    const url = new URL(location.href);
    url.searchParams.set("cd", String(cdId));
    history.replaceState(null, "", url);
}

// ---------- 再生状態 ----------
function setPlayState(playing) {
    els.playIcon.textContent = playing ? "pause" : "play_arrow";
    els.cover.classList.toggle("is-playing", playing);
    els.statusDot.classList.toggle("is-playing", playing);
    els.statusLabel.textContent = playing ? "NOW PLAYING" : "PAUSED";
    els.tracklist.classList.toggle("is-playing", playing);
    if ("mediaSession" in navigator) {
        navigator.mediaSession.playbackState = playing ? "playing" : "paused";
    }
}

// ---------- シーク操作 ----------
function seekFromEvent(e) {
    const rect = els.progressBar.getBoundingClientRect();
    const frac = (e.clientX - rect.left) / rect.width;
    engine.seek(frac);
}

let dragging = false;
els.progressBar.addEventListener("pointerdown", (e) => {
    dragging = true;
    els.progressBar.setPointerCapture(e.pointerId);
    seekFromEvent(e);
});
els.progressBar.addEventListener("pointermove", (e) => {
    if (dragging) seekFromEvent(e);
});
els.progressBar.addEventListener("pointerup", () => { dragging = false; });
els.progressBar.addEventListener("keydown", (e) => {
    const dur = engine.getDuration();
    if (!dur) return;
    if (e.key === "ArrowRight") engine.seek((engine.getPosition() + 5) / dur);
    if (e.key === "ArrowLeft") engine.seek((engine.getPosition() - 5) / dur);
});

// ---------- イベント配線 ----------
engine.on("trackchange", () => { updateNowPlaying(); });
engine.on("time", updateProgress);
engine.on("duration", updateDuration);
engine.on("playstate", setPlayState);
engine.on("error", () => {
    // 再生できないファイルは自動的に次へ
    setTimeout(() => engine.next(true), 300);
});

els.btnPlay.addEventListener("click", () => engine.toggle());
els.btnNext.addEventListener("click", () => engine.next(false));
els.btnPrev.addEventListener("click", () => engine.prev());
els.btnNextAlbum.addEventListener("click", () => goToAlbum(1));
els.btnPrevAlbum.addEventListener("click", () => goToAlbum(-1));
els.btnClose.addEventListener("click", () => window.close());

els.volume.addEventListener("input", () => {
    engine.setVolume(els.volume.value / 100);
    updateVolumeIcon();
});
els.btnMute.addEventListener("click", () => {
    engine.toggleMute();
    updateVolumeIcon();
});

function updateVolumeIcon() {
    const muted = engine.muted || engine.volume === 0;
    els.volIcon.textContent = muted ? "volume_off" : (engine.volume < 0.5 ? "volume_down" : "volume_up");
}

els.tracklistToggle.addEventListener("click", () => {
    const wrap = els.tracklist.closest(".player-tracklist-wrap");
    const collapsed = wrap.classList.toggle("collapsed");
    els.tracklistToggle.setAttribute("aria-expanded", String(!collapsed));
});

if ("mediaSession" in navigator) {
    navigator.mediaSession.setActionHandler("play", () => engine.play());
    navigator.mediaSession.setActionHandler("pause", () => engine.pause());
    navigator.mediaSession.setActionHandler("nexttrack", () => engine.next(false));
    navigator.mediaSession.setActionHandler("previoustrack", () => engine.prev());
}

// ---------- 起動 ----------
(async function init() {
    const cdId = getCdIdFromUrl();
    try {
        const res = await fetch("/api/cds");
        const cds = await res.json();
        allAlbums = buildAlbumList(cds);
    } catch {
        allAlbums = [];
    }

    if (allAlbums.length === 0) {
        els.trackTitle.textContent = "再生できるアルバムがありません";
        els.albumTitle.textContent = "音声ファイル付きのCDを登録してください";
        return;
    }

    const target = allAlbums.some((c) => c.id === cdId) ? cdId : allAlbums[0].id;
    await loadAlbum(target, null, true);
})();
