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
                <span class="player-topbar-spacer"></span>
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
    let currentCd = null;     // 再生中のアルバム (engine のキュー元)
    let viewCd = null;        // フル表示中のアルバム
    let viewTrackId = null;   // フル表示で選択/再生中のトラック
    let viewMode = "idle";    // 'live' | 'queued' | 'idle'
    let stagedCd = null;      // Up Next 予約
    let stagedTrackId = null;
    let fullVisible = false;
    let ambientFlip = false;

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
    function parseDur(str) {
        if (!str) return 0;
        const p = String(str).split(":").map((x) => parseInt(x, 10));
        if (p.some((x) => isNaN(x))) return 0;
        return p.length === 3 ? p[0] * 3600 + p[1] * 60 + p[2] : p[0] * 60 + p[1];
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
            </li>`;
        }
        el.tracklist.innerHTML = html || '<li class="player-track-empty">トラックがありません</li>';
        el["tracklist-count"].textContent = `${playableTracks(cd).length} 曲`;
    }

    // ---------- フル表示描画 ----------
    function renderView() {
        if (!viewCd) return;
        const track = trackById(viewCd, viewTrackId) || firstPlayable(viewCd);
        viewTrackId = track ? track.id : viewTrackId;
        paintCover(viewCd);
        el["track-title"].textContent = track ? track.title : viewCd.title;
        el["album-title"].textContent = viewCd.title;
        el["artist-name"].textContent = viewCd.artist || "";
        renderTracklist();

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
        el["mini-title"].textContent = t ? t.title : "—";
        el["mini-artist"].textContent = currentCd ? (currentCd.artist || currentCd.title) : "";
        if (currentCd) paintCover(currentCd);
        const playing = engine.isPlaying;
        el["mini-play-icon"].textContent = playing ? "pause" : "play_arrow";
        if (stagedCd) {
            el["mini-next-text"].textContent = stagedCd.title;
            el["mini-next"].hidden = false;
        } else {
            el["mini-next"].hidden = true;
        }
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

    // ---------- 再生開始 (ビューのアルバムから) ----------
    function startViewFromTrack(trackId) {
        if (!viewCd) return;
        const t = trackById(viewCd, trackId) || firstPlayable(viewCd);
        if (!t) return;
        currentCd = viewCd;
        viewTrackId = t.id;
        viewMode = "live";
        stagedCd = null; stagedTrackId = null;
        engine.loadTracks(viewCd.tracks, t.id);
        engine.play();
        renderView();
        renderMini();
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
        viewTrackId = (firstPlayable(cd) || {}).id || null;
        viewMode = computeViewMode();
        if (viewMode === "queued") { stagedCd = cd; stagedTrackId = null; }
        else if (viewMode === "idle") { stagedCd = null; stagedTrackId = null; }
        renderView();
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
        renderMini();
        updateMediaSession();
        if (fullVisible && viewMode === "live") { viewTrackId = t.id; renderView(); }
        else if (fullVisible && viewMode === "queued") { renderView(); } // バナーの現在再生中を更新
    });
    engine.on("time", (pos) => { setMiniProgress(pos); if (fullVisible && viewMode === "live") setViewProgress(pos); });
    engine.on("duration", (dur) => { if (fullVisible && viewMode === "live") el["time-total"].textContent = formatTime(dur); });
    engine.on("playstate", (playing) => {
        el["mini-play-icon"].textContent = playing ? "pause" : "play_arrow";
        rootEl.classList.toggle("is-playing", playing);
        if (fullVisible && viewMode === "live") {
            el["play-icon"].textContent = playing ? "pause" : "play_arrow";
            el.cover.classList.toggle("is-playing", playing);
            el["status-dot"].classList.toggle("is-playing", playing);
            el["status-label"].textContent = playing ? "NOW PLAYING" : "PAUSED";
            el.tracklist.classList.toggle("is-playing", playing);
        }
    });
    engine.on("ended", () => {
        if (stagedCd) {
            const cd = stagedCd; const tid = stagedTrackId;
            stagedCd = null; stagedTrackId = null;
            const t = (tid != null ? trackById(cd, tid) : null) || firstPlayable(cd);
            currentCd = cd;
            viewCd = cd; viewTrackId = t ? t.id : null; viewMode = "live";
            engine.loadTracks(cd.tracks, t ? t.id : null);
            engine.play();
            renderView(); renderMini();
        } else {
            engine.next(true);
        }
    });
    engine.on("error", () => { setTimeout(() => engine.next(true), 300); });

    el.tracklist.addEventListener("click", (e) => {
        const li = e.target.closest(".player-track");
        if (!li || li.classList.contains("player-track--disabled")) return;
        const id = parseInt(li.dataset.trackId, 10);
        if (viewMode === "live") engine.playTrackById(id);
        else startViewFromTrack(id);
    });

    rootEl.addEventListener("click", (e) => {
        const btn = e.target.closest("[data-act]");
        if (!btn) return;
        const act = btn.dataset.act;
        if (act === "back") api.hide();
        else if (act === "expand") api.expandNowPlaying();
        else if (act === "close") api.close();
        else if (act === "play") {
            if (viewMode === "live") engine.toggle();
            else startViewFromTrack(viewTrackId);
        }
        else if (act === "next") { if (viewMode === "live") engine.next(false); else viewStepTrack(1); }
        else if (act === "prev") { if (viewMode === "live") engine.prev(); else viewStepTrack(-1); }
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
        navigator.mediaSession.setActionHandler("nexttrack", () => engine.next(false));
        navigator.mediaSession.setActionHandler("previoustrack", () => engine.prev());
    }

    const api = {
        engine,
        setAlbums(list) { albums = list || []; },
        // autoplay=false: 閲覧/予約のみ (自動再生しない)。true: 即再生。
        openAlbum(cdId, autoplay) {
            const cd = albums.find((c) => c.id === cdId);
            if (!cd) return;
            if (autoplay) {
                viewCd = cd; viewTrackId = (firstPlayable(cd) || {}).id || null;
                currentCd = cd; viewMode = "live";
                stagedCd = null; stagedTrackId = null;
                engine.loadTracks(cd.tracks, viewTrackId);
                engine.play();
                renderView(); renderMini();
            } else {
                browseAlbum(cd);
            }
            api.show();
        },
        show() {
            if (!viewCd) return;
            renderView();
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
            if (engine.current()) { setVisibility("mini"); renderMini(); }
            else api.close();
        },
        close() {
            engine.pause();
            stagedCd = null; stagedTrackId = null;
            setVisibility("closed");
            renderMini();
        },
    };

    return api;
}
