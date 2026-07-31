// 再利用可能なプレイヤー UI コンポーネント。
//
// 状態モデル:
//   viewMode = 'live'   … フル表示中のアルバム = 再生中のアルバム (進行/ハイライトはライブ)
//            'queued'   … 別のアルバムを再生中に閲覧中 = 「Up Next」予約 (再生は継続)
//            'idle'     … 何も再生していない状態で閲覧中 (プレビュー)
//   表示     = 'full' / 'mini' / 'closed'
//
// 挙動:
//   ・ライブラリのカードクリックは自動再生しない (閲覧/予約のみ)。
//   ・再生は「再生ボタン」または「トラッククリック」で初めて始まる。
//   ・再生中に別CDを開くと Up Next として予約され、現在の曲が終わると再生される。
//   ・一覧に戻ると下部ミニバーで再生継続。ミニバークリックでフル表示へ。

function createPlayerUI(rootEl) {
    rootEl.classList.add("player-root", "closed");
    rootEl.innerHTML = `
    <div class="player-overlay">
        <div class="player-ambient" aria-hidden="true">
            <div class="player-ambient-img" data-ambient="a"></div>
            <div class="player-ambient-img" data-ambient="b"></div>
            <div class="player-ambient-shade"></div>
        </div>
        <div class="player-shell">
            <header class="player-topbar">
                <button class="player-iconbtn player-back" data-act="back" aria-label="一覧に戻る" title="一覧に戻る">
                    <span class="material-icons">arrow_back</span>
                </button>
                <span class="player-eyebrow">
                    <span class="player-eyebrow-dot" data-el="status-dot"></span>
                    <span data-el="status-label">NOW PLAYING</span>
                </span>
                <button class="player-iconbtn player-queue-topbtn" data-act="toggle-queue" aria-label="再生キューを開く" title="再生キュー">
                    <span class="material-icons">queue_music</span>
                </button>
            </header>
            <main class="player-main">
                <section class="player-coverwrap">
                    <div class="player-cover" data-el="cover">
                        <img data-el="cover-img" src="" alt="" draggable="false">
                        <div class="player-cover-fallback" data-el="cover-fallback">
                            <span class="material-icons">album</span>
                        </div>
                    </div>
                </section>
                <section class="player-info">
                    <div class="player-nowplaying-banner" data-el="banner" hidden>
                        <span class="material-icons">play_circle</span>
                        <span>現在再生中: <strong data-el="banner-text"></strong></span>
                    </div>
                    <div class="player-titles">
                        <h2 class="player-track-title" data-el="track-title">—</h2>
                        <div class="player-album" data-el="album-title">—</div>
                        <div class="player-artist" data-el="artist-name">—</div>
                    </div>
                    <div class="player-tech" data-el="tech" aria-label="音声情報">
                        <span class="player-tech-chip"><span class="material-icons">graphic_eq</span><strong data-el="tech-format">AUDIO</strong></span>
                        <span class="player-tech-chip"><span class="material-icons">data_usage</span><strong data-el="tech-size">—</strong></span>
                        <span class="player-tech-chip player-tech-chip--encoder" data-el="tech-encoder-wrap"><span class="material-icons">memory</span><strong data-el="tech-encoder">—</strong></span>
                    </div>
                    <div class="player-album-actions">
                        <button class="player-secondary-btn player-resume-btn" data-act="resume-audiobook" data-el="resume-button" hidden aria-label="オーディオブックを続きから再生" title="オーディオブックを続きから再生">
                            <span class="material-icons">play_circle</span><span data-el="resume-label">続きから再生</span>
                        </button>
                        <button class="player-secondary-btn" data-act="add-album" aria-label="このCDを再生キューに追加" title="このCDを再生キューに追加">
                            <span class="material-icons">playlist_add</span><span>CDをキューに追加</span>
                        </button>
                    </div>
                    <div class="player-progress">
                        <div class="player-progress-bar" data-el="progress-bar" role="slider" aria-label="再生位置" tabindex="0">
                            <div class="player-progress-fill" data-el="progress-fill"></div>
                            <div class="player-progress-thumb" data-el="progress-thumb"></div>
                        </div>
                        <div class="player-times">
                            <span data-el="time-current">0:00</span>
                            <span data-el="time-total">0:00</span>
                        </div>
                    </div>
                    <div class="player-controls">
                        <button class="player-iconbtn player-albumnav" data-act="prev-album" aria-label="前のアルバム" title="前のアルバム">
                            <span class="material-icons">album</span>
                            <span class="player-albumnav-chevron material-icons">chevron_left</span>
                        </button>
                        <button class="player-iconbtn player-ctrl" data-act="prev" aria-label="前の曲" title="前の曲">
                            <span class="material-icons">skip_previous</span>
                        </button>
                        <button class="player-playbtn" data-act="play" aria-label="再生/一時停止" title="再生/一時停止">
                            <span class="material-icons" data-el="play-icon">play_arrow</span>
                        </button>
                        <button class="player-iconbtn player-ctrl" data-act="next" aria-label="次の曲" title="次の曲">
                            <span class="material-icons">skip_next</span>
                        </button>
                        <button class="player-iconbtn player-albumnav" data-act="next-album" aria-label="次のアルバム" title="次のアルバム">
                            <span class="material-icons">album</span>
                            <span class="player-albumnav-chevron material-icons">chevron_right</span>
                        </button>
                    </div>
                    <div class="player-secondary-controls">
                        <button class="player-secondary-btn" data-act="shuffle" data-el="shuffle-button" aria-pressed="false" aria-label="シャッフル" title="シャッフル">
                            <span class="material-icons">shuffle</span><span>シャッフル</span>
                        </button>
                        <button class="player-secondary-btn" data-act="repeat" data-el="repeat-button" aria-pressed="true" aria-label="キューをループ" title="ループモード">
                            <span class="material-icons" data-el="repeat-icon">repeat</span><span data-el="repeat-label">キュー</span>
                        </button>
                        <button class="player-secondary-btn" data-act="toggle-queue" aria-label="再生キューを開く" title="再生キュー">
                            <span class="material-icons">queue_music</span><span>再生キュー</span>
                        </button>
                    </div>
                    <div class="player-volume">
                        <button class="player-iconbtn player-volbtn" data-act="mute" aria-label="ミュート" title="ミュート">
                            <span class="material-icons" data-el="vol-icon">volume_up</span>
                        </button>
                        <input type="range" data-el="volume" class="player-volslider" min="0" max="100" value="100" aria-label="音量">
                    </div>
                    <div class="player-tracklist-wrap">
                        <button class="player-tracklist-toggle" data-act="toggle-tracklist" aria-expanded="true">
                            <span class="player-tracklist-toggle-label">トラックリスト</span>
                            <span class="player-tracklist-count" data-el="tracklist-count"></span>
                            <span class="material-icons player-tracklist-caret">expand_more</span>
                        </button>
                        <ol class="player-tracklist" data-el="tracklist"></ol>
                    </div>
                </section>
            </main>
        </div>
        <div class="player-queue-backdrop" data-act="close-queue" aria-hidden="true"></div>
        <aside class="player-queue" data-el="queue-panel" aria-label="再生キュー" aria-hidden="true">
            <header class="player-queue-header">
                <div>
                    <span class="player-queue-kicker">PLAY QUEUE</span>
                    <h2>再生キュー</h2>
                </div>
                <div class="player-queue-header-actions">
                    <button class="player-queue-clear" data-act="clear-queue" aria-label="キューを空にする">クリア</button>
                    <button class="player-iconbtn" data-act="close-queue" aria-label="再生キューを閉じる" title="閉じる">
                        <span class="material-icons">close</span>
                    </button>
                </div>
            </header>
            <ol class="player-queue-list" data-el="queue-list"></ol>
        </aside>
    </div>
    <div class="player-mini">
        <div class="player-mini-progress"><div class="player-mini-progress-fill" data-el="mini-fill"></div></div>
        <div class="player-mini-body">
            <button class="player-mini-coverbtn" data-act="expand" aria-label="プレイヤーを開く">
                <img class="player-mini-cover" data-el="mini-cover" src="" alt="" draggable="false">
                <span class="player-mini-coverfallback" data-el="mini-cover-fallback"><span class="material-icons">album</span></span>
            </button>
            <div class="player-mini-text" data-act="expand">
                <div class="player-mini-title" data-el="mini-title">—</div>
                <div class="player-mini-artist" data-el="mini-artist">—</div>
                <div class="player-mini-next" data-el="mini-next" hidden>次に: <span data-el="mini-next-text"></span></div>
            </div>
            <div class="player-mini-controls">
                <button class="player-mini-btn" data-act="prev" aria-label="前の曲" title="前の曲"><span class="material-icons">skip_previous</span></button>
                <button class="player-mini-btn player-mini-play" data-act="play" aria-label="再生/一時停止" title="再生/一時停止"><span class="material-icons" data-el="mini-play-icon">play_arrow</span></button>
                <button class="player-mini-btn" data-act="next" aria-label="次の曲" title="次の曲"><span class="material-icons">skip_next</span></button>
                <button class="player-mini-btn" data-act="toggle-queue" aria-label="再生キューを開く" title="再生キュー"><span class="material-icons">queue_music</span></button>
                <button class="player-mini-btn player-mini-close" data-act="close" aria-label="再生を停止" title="再生を停止"><span class="material-icons">close</span></button>
            </div>
        </div>
    </div>`;

    const $ = (sel) => rootEl.querySelector(sel);
    const el = {};
    rootEl.querySelectorAll("[data-el]").forEach((n) => { el[n.dataset.el] = n; });
    const ambientA = $('[data-ambient="a"]');
    const ambientB = $('[data-ambient="b"]');

    const engine = new PlayerEngine();

    let albums = [];
    let currentCd = null;     // 現在再生中の曲が属するアルバム
    let viewCd = null;        // フル表示中のアルバム
    let viewTrackId = null;   // フル表示で選択/再生中のトラック
    let viewMode = "idle";    // 'live' | 'queued' | 'idle'
    let fullVisible = false;
    let ambientFlip = false;
    let queueVisible = false;
    let metadataRequestId = 0;
    const metadataCache = new Map();
    const AUDIOBOOK_PROGRESS_KEY = "dantalian_audiobook_progress_v1";
    let audiobookProgress = loadAudiobookProgress();
    let pendingAudiobookResume = null;
    let lastProgressSaveAt = 0;

    // ---------- ユーティリティ ----------
    function formatTime(sec) {
        if (!isFinite(sec) || sec < 0) sec = 0;
        const m = Math.floor(sec / 60);
        const s = Math.floor(sec % 60);
        return `${m}:${String(s).padStart(2, "0")}`;
    }
    function coverUrl(cd) { return cd && cd.cover_url ? `/images/${cd.cover_url}` : null; }
    function playableTracks(cd) { return (cd.tracks || []).filter((t) => t.file_hash); }
    function firstPlayable(cd) { return playableTracks(cd)[0] || null; }
    function trackById(cd, id) { return (cd.tracks || []).find((t) => t.id === id) || null; }
    function isAudiobook(cd) { return !!cd && cd.media_type === "audiobook"; }
    function loadAudiobookProgress() {
        try {
            if (typeof localStorage === "undefined") return {};
            const value = JSON.parse(localStorage.getItem(AUDIOBOOK_PROGRESS_KEY) || "{}");
            return value && typeof value === "object" && !Array.isArray(value) ? value : {};
        } catch {
            return {};
        }
    }
    function getAudiobookProgress(cd) {
        if (!isAudiobook(cd)) return null;
        const saved = audiobookProgress[String(cd.id)];
        if (!saved) return null;
        const track = playableTracks(cd).find((item) => item.id === Number(saved.trackId));
        const position = Number(saved.position);
        if (!track || !Number.isFinite(position) || position < 0) return null;
        return { track, position };
    }
    function saveAudiobookProgress(force = false) {
        const entry = engine.currentEntry();
        const cd = entry && entry.album;
        if (!isAudiobook(cd)) return false;
        const now = Date.now();
        if (!force && now - lastProgressSaveAt < 1000) return false;
        audiobookProgress[String(cd.id)] = {
            trackId: entry.track.id,
            position: Math.max(0, engine.getPosition()),
            updatedAt: now,
        };
        lastProgressSaveAt = now;
        try {
            if (typeof localStorage !== "undefined") {
                localStorage.setItem(AUDIOBOOK_PROGRESS_KEY, JSON.stringify(audiobookProgress));
            }
        } catch {}
        return true;
    }
    function applyPendingAudiobookResume() {
        const pending = pendingAudiobookResume;
        const entry = engine.currentEntry();
        if (!pending || !entry || !entry.album || entry.album.id !== pending.cdId || entry.track.id !== pending.trackId) return false;
        if (engine.getDuration() <= 0) return false;
        pendingAudiobookResume = null;
        engine.setPosition(pending.position);
        saveAudiobookProgress(true);
        return true;
    }
    function parseDur(str) {
        if (!str) return 0;
        const p = String(str).split(":").map((x) => parseInt(x, 10));
        if (p.some((x) => isNaN(x))) return 0;
        return p.length === 3 ? p[0] * 3600 + p[1] * 60 + p[2] : p[0] * 60 + p[1];
    }
    function formatBytes(bytes) {
        const value = Number(bytes);
        if (!Number.isFinite(value) || value <= 0) return "—";
        if (value >= 1024 ** 3) return `${(value / (1024 ** 3)).toFixed(1)} GB`;
        if (value >= 1024 ** 2) return `${(value / (1024 ** 2)).toFixed(1)} MB`;
        return `${Math.max(1, Math.round(value / 1024))} KB`;
    }
    function fallbackFileType(track) {
        const name = track && track.file_name ? track.file_name : "";
        const ext = name.includes(".") ? name.split(".").pop() : "audio";
        return ext.toUpperCase();
    }
    function paintTrackTechnicalInfo(track, metadata) {
        const fileType = metadata && metadata.file_type
            ? String(metadata.file_type).toUpperCase()
            : fallbackFileType(track);
        el["tech-format"].textContent = fileType;
        el["tech-size"].textContent = formatBytes(metadata && metadata.raw_size_bytes);
        const encoder = metadata && metadata.encoder ? String(metadata.encoder) : "";
        el["tech-encoder"].textContent = encoder || "—";
        el["tech-encoder-wrap"].hidden = !encoder;
    }
    async function loadTrackTechnicalInfo(cd, track) {
        const requestId = ++metadataRequestId;
        if (!track) {
            el.tech.hidden = true;
            return;
        }
        el.tech.hidden = false;
        paintTrackTechnicalInfo(track, metadataCache.get(track.id));
        if (metadataCache.has(track.id) || !cd || !cd.id) return;
        try {
            const res = await fetch(`/api/cds/${cd.id}/tracks/${track.id}/metadata`, { cache: "no-store" });
            if (!res.ok) return;
            const metadata = await res.json();
            metadataCache.set(track.id, metadata || {});
            if (requestId === metadataRequestId) paintTrackTechnicalInfo(track, metadata);
        } catch {}
    }
    function computeViewMode() {
        if (!viewCd) return "idle";
        if (currentCd && currentCd.id === viewCd.id) return "live";
        if (currentCd) return "queued";
        return "idle";
    }

    // ---------- アンビエント / カバー ----------
    function setAmbient(url) {
        const show = ambientFlip ? ambientA : ambientB;
        const hide = ambientFlip ? ambientB : ambientA;
        if (url) {
            show.style.backgroundImage = `url("${url}")`;
            show.classList.add("active");
            hide.classList.remove("active");
            ambientFlip = !ambientFlip;
        } else {
            ambientA.classList.remove("active");
            ambientB.classList.remove("active");
        }
    }
    function paintCover(cd) {
        const url = coverUrl(cd);
        if (url) {
            el["cover-img"].src = url; el["cover-img"].alt = cd.title || "";
            el["cover-img"].style.display = "block"; el["cover-fallback"].style.display = "none";
            el["mini-cover"].src = url; el["mini-cover"].style.display = "block";
            el["mini-cover-fallback"].style.display = "none";
        } else {
            el["cover-img"].removeAttribute("src"); el["cover-img"].style.display = "none";
            el["cover-fallback"].style.display = "flex";
            el["mini-cover"].removeAttribute("src"); el["mini-cover"].style.display = "none";
            el["mini-cover-fallback"].style.display = "flex";
        }
        setAmbient(url);
    }

    // ---------- トラックリスト ----------
    function renderTracklist() {
        const cd = viewCd;
        if (!cd) { el.tracklist.innerHTML = ""; return; }
        const tracks = cd.tracks || [];
        const multiDisc = (cd.disc_count || 1) > 1 || tracks.some((t) => (t.disc_number || 1) > 1);
        let html = "";
        let lastDisc = null;
        for (const t of tracks) {
            const disc = t.disc_number || 1;
            if (multiDisc && disc !== lastDisc) { html += `<li class="player-tracklist-disc">DISC ${disc}</li>`; lastDisc = disc; }
            const playable = !!t.file_hash;
            const num = String(t.track_number).padStart(2, "0");
            html += `
            <li class="player-track${playable ? "" : " player-track--disabled"}${t.id === viewTrackId ? " player-track--current" : ""}" data-track-id="${t.id}">
                <span class="player-track-num">
                    <span class="player-track-num-text">${num}</span>
                    <span class="player-eq" aria-hidden="true"><i></i><i></i><i></i></span>
                </span>
                <span class="player-track-title-cell">${escapeHtml(t.title)}</span>
                <span class="player-track-dur">${t.duration ? escapeHtml(t.duration) : (playable ? "" : "—")}</span>
                ${playable ? `<button class="player-track-add" data-act="add-track" data-track-id="${t.id}" aria-label="${escapeAttr(t.title)}をキューに追加" title="キューに追加"><span class="material-icons">playlist_add</span></button>` : ""}
            </li>`;
        }
        el.tracklist.innerHTML = html || '<li class="player-track-empty">トラックがありません</li>';
        el["tracklist-count"].textContent = `${playableTracks(cd).length} 曲`;
    }

    function renderQueue() {
        const entries = engine.queue || [];
        if (entries.length === 0) {
            el["queue-list"].innerHTML = '<li class="player-queue-empty">再生中のトラックはありません</li>';
            return;
        }

        const orderedIndexes = engine.playOrder.length
            ? engine.playOrder
            : entries.map((_, index) => index);
        const currentPosition = orderedIndexes.indexOf(engine.index);
        let html = "";
        let previousAlbumId = null;
        orderedIndexes.forEach((queueIndex, displayIndex) => {
            const entry = entries[queueIndex];
            const track = entry.track;
            const album = entry.album;
            const albumId = album ? album.id : "unknown";
            if (albumId !== previousAlbumId) {
                const label = displayIndex === currentPosition ? "NOW PLAYING" : (displayIndex > currentPosition ? "UP NEXT" : "PLAYED");
                html += `<li class="player-queue-section"><span>${label}</span><strong>${escapeHtml(album ? album.title : "音声ファイル")}</strong></li>`;
                previousAlbumId = albumId;
            }
            const active = queueIndex === engine.index;
            html += `
            <li class="player-queue-track${active ? " is-current" : ""}" data-queue-index="${queueIndex}">
                <span class="player-queue-track-index">${active ? '<span class="material-icons">equalizer</span>' : String(displayIndex + 1).padStart(2, "0")}</span>
                <span class="player-queue-track-copy">
                    <strong>${escapeHtml(track.title)}</strong>
                    <small>${escapeHtml(album ? album.title : (track.file_name || "音声ファイル"))}</small>
                </span>
                <span class="player-queue-track-duration">${track.duration ? escapeHtml(track.duration) : "—"}</span>
                <button class="player-queue-remove" data-act="remove-queue" data-queue-index="${queueIndex}" aria-label="${escapeAttr(track.title)}をキューから削除" title="キューから削除"><span class="material-icons">close</span></button>
            </li>`;
        });
        el["queue-list"].innerHTML = html;
        el["queue-list"].classList.toggle("is-playing", engine.isPlaying);
    }

    // ---------- フル表示描画 ----------
    function renderView() {
        if (!viewCd) return;
        const track = trackById(viewCd, viewTrackId) || firstPlayable(viewCd);
        viewTrackId = track ? track.id : viewTrackId;
        const resume = getAudiobookProgress(viewCd);
        const showResume = !!resume && viewMode !== "live";
        el["resume-button"].hidden = !showResume;
        if (showResume) {
            el["resume-label"].textContent = `続きから再生 · ${resume.track.title} (${formatTime(resume.position)})`;
        }
        paintCover(viewCd);
        el["track-title"].textContent = track ? track.title : viewCd.title;
        el["album-title"].textContent = viewCd.title;
        el["artist-name"].textContent = viewCd.artist || "";
        renderTracklist();
        loadTrackTechnicalInfo(viewCd, track);

        rootEl.classList.remove("view-live", "view-queued", "view-idle");
        rootEl.classList.add(`view-${viewMode}`);

        if (viewMode === "live") {
            el.banner.hidden = true;
            const playing = engine.isPlaying;
            el["play-icon"].textContent = playing ? "pause" : "play_arrow";
            el.cover.classList.toggle("is-playing", playing);
            el["status-dot"].classList.toggle("is-playing", playing);
            el["status-label"].textContent = playing ? "NOW PLAYING" : "PAUSED";
            el.tracklist.classList.toggle("is-playing", playing);
            setViewProgress(engine.getPosition());
            const dur = engine.getDuration();
            el["time-total"].textContent = formatTime(dur > 0 ? dur : parseDur(track && track.duration));
        } else {
            // queued / idle: プレビュー状態 (再生はビューのアルバムからは流れていない)
            el["play-icon"].textContent = "play_arrow";
            el.cover.classList.remove("is-playing");
            el.tracklist.classList.remove("is-playing");
            el["status-dot"].classList.remove("is-playing");
            el["status-label"].textContent = viewMode === "queued" ? "UP NEXT" : "PREVIEW";
            setViewProgress(0);
            el["time-total"].textContent = formatTime(parseDur(track && track.duration));
            if (viewMode === "queued" && currentCd) {
                const ct = engine.current();
                el["banner-text"].textContent = `${currentCd.title}${ct ? " / " + ct.title : ""}`;
                el.banner.hidden = false;
            } else {
                el.banner.hidden = true;
            }
        }
    }

    function setViewProgress(pos) {
        const dur = engine.getDuration();
        const frac = (viewMode === "live" && dur > 0) ? Math.min(pos / dur, 1) : 0;
        const pct = (frac * 100).toFixed(3) + "%";
        el["progress-fill"].style.width = pct;
        el["progress-thumb"].style.left = pct;
        el["time-current"].textContent = viewMode === "live" ? formatTime(pos) : "0:00";
    }

    // ---------- ミニ描画 (常に再生中を反映) ----------
    function renderMini() {
        const t = engine.current();
        const entry = engine.currentEntry();
        if (entry && entry.album) currentCd = entry.album;
        el["mini-title"].textContent = t ? t.title : "—";
        el["mini-artist"].textContent = currentCd ? (currentCd.artist || currentCd.title) : "";
        if (currentCd) paintCover(currentCd);
        const playing = engine.isPlaying;
        el["mini-play-icon"].textContent = playing ? "pause" : "play_arrow";
        el["mini-next"].hidden = true;
        renderQueue();
    }
    function setMiniProgress(pos) {
        const dur = engine.getDuration();
        const frac = dur > 0 ? Math.min(pos / dur, 1) : 0;
        el["mini-fill"].style.width = (frac * 100).toFixed(3) + "%";
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

    // ---------- 表示切替 ----------
    function setVisibility(v) {
        rootEl.classList.remove("full", "mini", "closed");
        rootEl.classList.add(v);
        document.body.classList.remove("player-active", "player-mini-active");
        fullVisible = (v === "full");
        if (v === "full") document.body.classList.add("player-active");
        else if (v === "mini") document.body.classList.add("player-mini-active");
    }

    function setQueueVisibility(visible) {
        queueVisible = Boolean(visible);
        rootEl.classList.toggle("queue-open", queueVisible);
        el["queue-panel"].setAttribute("aria-hidden", String(!queueVisible));
        if (queueVisible) renderQueue();
    }

    // ---------- 再生開始 (ビューのアルバムから) ----------
    function startAlbumFromTrack(cd, trackId, options = {}) {
        if (!cd) return;
        const saved = isAudiobook(cd) ? getAudiobookProgress(cd) : null;
        const preferSaved = !!options.preferSaved && !!saved;
        const t = preferSaved ? saved.track : (trackById(cd, trackId) || firstPlayable(cd));
        if (!t) return;
        const resumePosition = Number.isFinite(options.resumePosition)
            ? Math.max(0, options.resumePosition)
            : (preferSaved ? saved.position : 0);
        saveAudiobookProgress(true);
        currentCd = cd;
        viewCd = cd;
        viewTrackId = t.id;
        viewMode = "live";
        pendingAudiobookResume = isAudiobook(cd) && resumePosition > 0
            ? { cdId: cd.id, trackId: t.id, position: resumePosition }
            : null;
        const entries = playableTracks(cd).map((track) => ({ track, album: cd }));
        const startIndex = entries.findIndex((entry) => entry.track.id === t.id);
        engine.loadQueue(entries, startIndex >= 0 ? startIndex : 0);
        if (!pendingAudiobookResume) saveAudiobookProgress(true);
        engine.play();
        applyPendingAudiobookResume();
        renderView();
        renderMini();
        renderQueue();
    }

    function startViewFromTrack(trackId, options = {}) {
        if (!viewCd) return;
        startAlbumFromTrack(viewCd, trackId, options);
    }

    function appendAlbumToQueue(cd) {
        if (!cd) return;
        const entries = playableTracks(cd).map((track) => ({ track, album: cd }));
        if (entries.length === 0) return;
        if (engine.queue.length === 0) {
            currentCd = cd;
            viewCd = cd;
            viewTrackId = entries[0].track.id;
            viewMode = "live";
            engine.loadQueue(entries, 0);
        } else {
            engine.appendQueue(entries);
        }
        if (fullVisible) renderView();
        renderQueue();
    }

    function appendTrackToQueue(cd, trackId) {
        const track = trackById(cd, trackId);
        if (!cd || !track || !track.file_hash) return;
        if (engine.queue.length === 0) {
            currentCd = cd;
            viewCd = cd;
            viewTrackId = track.id;
            viewMode = "live";
            engine.loadQueue([{ track, album: cd }], 0);
        } else {
            engine.appendQueue([{ track, album: cd }]);
        }
        if (fullVisible) renderView();
        renderQueue();
    }

    function viewStepTrack(delta) {
        if (!viewCd) return;
        const list = playableTracks(viewCd);
        if (list.length === 0) return;
        let i = list.findIndex((t) => t.id === viewTrackId);
        i = (i < 0 ? 0 : (i + delta + list.length) % list.length);
        startViewFromTrack(list[i].id);
    }

    // ---------- アルバム閲覧 ----------
    function browseAlbum(cd) {
        if (!cd) return;
        viewCd = cd;
        const resume = getAudiobookProgress(cd);
        viewTrackId = (resume ? resume.track : firstPlayable(cd))?.id || null;
        viewMode = computeViewMode();
        renderView();
        renderQueue();
    }

    function navAlbum(delta) {
        if (albums.length === 0 || !viewCd) return;
        let idx = albums.findIndex((c) => c.id === viewCd.id);
        if (idx < 0) idx = 0;
        const next = (idx + delta + albums.length) % albums.length;
        browseAlbum(albums[next]);
    }

    // ---------- シーク ----------
    function seekFromEvent(e) {
        if (viewMode !== "live") return;
        const rect = el["progress-bar"].getBoundingClientRect();
        engine.seek((e.clientX - rect.left) / rect.width);
    }
    let dragging = false;
    el["progress-bar"].addEventListener("pointerdown", (e) => {
        if (viewMode !== "live") return;
        dragging = true; el["progress-bar"].setPointerCapture(e.pointerId); seekFromEvent(e);
    });
    el["progress-bar"].addEventListener("pointermove", (e) => { if (dragging) seekFromEvent(e); });
    el["progress-bar"].addEventListener("pointerup", () => { dragging = false; });
    el["progress-bar"].addEventListener("keydown", (e) => {
        if (viewMode !== "live") return;
        const dur = engine.getDuration();
        if (!dur) return;
        if (e.key === "ArrowRight") engine.seek((engine.getPosition() + 5) / dur);
        if (e.key === "ArrowLeft") engine.seek((engine.getPosition() - 5) / dur);
    });

    // ---------- 音量 ----------
    function updateVolumeIcon() {
        const muted = engine.muted || engine.volume === 0;
        el["vol-icon"].textContent = muted ? "volume_off" : (engine.volume < 0.5 ? "volume_down" : "volume_up");
    }
    el.volume.addEventListener("input", () => { engine.setVolume(el.volume.value / 100); updateVolumeIcon(); });

    // ---------- エンジンイベント ----------
    engine.on("trackchange", (t) => {
        const entry = engine.currentEntry();
        if (entry && entry.album) currentCd = entry.album;
        const pending = pendingAudiobookResume;
        const isPendingResume = pending && entry && entry.album
            && pending.cdId === entry.album.id && pending.trackId === entry.track.id;
        if (!isPendingResume) saveAudiobookProgress(true);
        renderMini();
        renderQueue();
        updateMediaSession();
        if (fullVisible && viewMode === "live") { viewTrackId = t.id; renderView(); }
        else if (fullVisible && viewMode === "queued") { renderView(); } // バナーの現在再生中を更新
    });
    engine.on("queuechange", () => renderQueue());
    engine.on("empty", () => {
        currentCd = null;
        if (fullVisible && viewCd) {
            viewMode = "idle";
            renderView();
        }
        renderMini();
        renderQueue();
    });
    engine.on("time", (pos) => {
        saveAudiobookProgress();
        setMiniProgress(pos);
        if (fullVisible && viewMode === "live") setViewProgress(pos);
    });
    engine.on("duration", (dur) => {
        const restored = applyPendingAudiobookResume();
        if (fullVisible && viewMode === "live") {
            el["time-total"].textContent = formatTime(dur);
            if (restored) setViewProgress(engine.getPosition());
        }
    });
    engine.on("shuffle", (enabled) => {
        el["shuffle-button"].classList.toggle("is-active", enabled);
        el["shuffle-button"].setAttribute("aria-pressed", String(enabled));
    });
    engine.on("repeat", (mode) => {
        const labels = { queue: "キュー", track: "1曲", off: "オフ" };
        el["repeat-button"].classList.toggle("is-active", mode !== "off");
        el["repeat-button"].setAttribute("aria-pressed", String(mode !== "off"));
        el["repeat-label"].textContent = labels[mode] || "オフ";
        el["repeat-icon"].textContent = mode === "track" ? "repeat_one" : "repeat";
    });
    engine.on("playstate", (playing) => {
        if (playing) applyPendingAudiobookResume();
        else saveAudiobookProgress(true);
        el["mini-play-icon"].textContent = playing ? "pause" : "play_arrow";
        rootEl.classList.toggle("is-playing", playing);
        el["queue-list"].classList.toggle("is-playing", playing);
        if (fullVisible && viewMode === "live") {
            el["play-icon"].textContent = playing ? "pause" : "play_arrow";
            el.cover.classList.toggle("is-playing", playing);
            el["status-dot"].classList.toggle("is-playing", playing);
            el["status-label"].textContent = playing ? "NOW PLAYING" : "PAUSED";
            el.tracklist.classList.toggle("is-playing", playing);
        }
    });
    engine.on("ended", () => {
        saveAudiobookProgress(true);
        engine.advanceAfterEnded();
        saveAudiobookProgress(true);
    });
    engine.on("error", () => {
        saveAudiobookProgress(true);
        setTimeout(() => engine.next(), 300);
    });

    el.tracklist.addEventListener("click", (e) => {
        if (e.target.closest("[data-act]")) return;
        const li = e.target.closest(".player-track");
        if (!li || li.classList.contains("player-track--disabled")) return;
        const id = parseInt(li.dataset.trackId, 10);
        if (viewMode === "live") {
            saveAudiobookProgress(true);
            engine.playTrackById(id);
        }
        else startViewFromTrack(id);
    });

    el["queue-list"].addEventListener("click", (e) => {
        if (e.target.closest("[data-act]")) return;
        const item = e.target.closest(".player-queue-track");
        if (!item) return;
        const queueIndex = parseInt(item.dataset.queueIndex, 10);
        const entry = engine.queue[queueIndex];
        saveAudiobookProgress(true);
        if (!entry || !engine.playQueueIndex(queueIndex)) return;
        currentCd = entry.album || currentCd;
        if (entry.album) {
            viewCd = entry.album;
            viewTrackId = entry.track.id;
            viewMode = "live";
            renderView();
        }
    });

    rootEl.addEventListener("click", (e) => {
        const btn = e.target.closest("[data-act]");
        if (!btn) return;
        const act = btn.dataset.act;
        if (act === "back") api.hide();
        else if (act === "expand") api.expandNowPlaying();
        else if (act === "close") api.close();
        else if (act === "toggle-queue") {
            if (!fullVisible) api.expandNowPlaying();
            setQueueVisibility(!queueVisible);
        }
        else if (act === "close-queue") setQueueVisibility(false);
        else if (act === "shuffle") engine.toggleShuffle();
        else if (act === "repeat") engine.toggleRepeatMode();
        else if (act === "add-album") appendAlbumToQueue(viewCd);
        else if (act === "add-track") appendTrackToQueue(viewCd, parseInt(btn.dataset.trackId, 10));
        else if (act === "remove-queue") engine.removeQueueIndex(parseInt(btn.dataset.queueIndex, 10));
        else if (act === "clear-queue") {
            engine.clearQueue();
            setQueueVisibility(false);
        }
        else if (act === "resume-audiobook") {
            const resume = getAudiobookProgress(viewCd);
            if (resume) startAlbumFromTrack(viewCd, resume.track.id, { resumePosition: resume.position });
        }
        else if (act === "play") {
            if (viewMode === "live") engine.toggle();
            else startViewFromTrack(viewTrackId, { preferSaved: isAudiobook(viewCd) });
        }
        else if (act === "next") {
            if (viewMode === "live") { saveAudiobookProgress(true); engine.next(); }
            else viewStepTrack(1);
        }
        else if (act === "prev") {
            if (viewMode === "live") { saveAudiobookProgress(true); engine.prev(); }
            else viewStepTrack(-1);
        }
        else if (act === "next-album") navAlbum(1);
        else if (act === "prev-album") navAlbum(-1);
        else if (act === "mute") { engine.toggleMute(); updateVolumeIcon(); }
        else if (act === "toggle-tracklist") {
            const wrap = btn.closest(".player-tracklist-wrap");
            const collapsed = wrap.classList.toggle("collapsed");
            btn.setAttribute("aria-expanded", String(!collapsed));
        }
    });

    if ("mediaSession" in navigator) {
        navigator.mediaSession.setActionHandler("play", () => engine.play());
        navigator.mediaSession.setActionHandler("pause", () => engine.pause());
        navigator.mediaSession.setActionHandler("nexttrack", () => { saveAudiobookProgress(true); engine.next(); });
        navigator.mediaSession.setActionHandler("previoustrack", () => { saveAudiobookProgress(true); engine.prev(); });
    }

    window.addEventListener("pagehide", () => saveAudiobookProgress(true));
    document.addEventListener("visibilitychange", () => {
        if (document.visibilityState === "hidden") saveAudiobookProgress(true);
    });

    const api = {
        engine,
        setAlbums(list) { albums = list || []; },
        // autoplay=false: 閲覧/予約のみ (自動再生しない)。true: 即再生。
        openAlbum(cdId, autoplay) {
            const cd = albums.find((c) => c.id === cdId);
            if (!cd) return;
            if (autoplay) {
                if (!firstPlayable(cd)) return;
                startAlbumFromTrack(cd, null, { preferSaved: isAudiobook(cd) });
            } else {
                browseAlbum(cd);
            }
            api.show();
        },
        show() {
            if (!viewCd) return;
            renderView();
            setQueueVisibility(false);
            setVisibility("full");
        },
        // ミニバー展開: 再生中アルバムをライブ表示にして開く
        expandNowPlaying() {
            if (currentCd) {
                viewCd = currentCd;
                viewTrackId = engine.current() ? engine.current().id : (firstPlayable(currentCd) || {}).id || null;
                viewMode = "live";
            }
            if (!viewCd) return;
            renderView();
            setVisibility("full");
        },
        hide() {
            setQueueVisibility(false);
            if (engine.current()) { setVisibility("mini"); renderMini(); }
            else api.close();
        },
        close() {
            saveAudiobookProgress(true);
            engine.pause();
            setQueueVisibility(false);
            setVisibility("closed");
            renderMini();
        },
    };

    return api;
}
