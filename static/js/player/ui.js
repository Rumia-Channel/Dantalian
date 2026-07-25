// 再利用可能なプレイヤー UI コンポーネント。
// 指定コンテナ内に「フルプレイヤー(オーバーレイ)」と「ミニプレイヤー(下部バー)」を描画し、
// PlayerEngine と結びつける。
//
// 状態:
//   hidden   … どちらも非表示 (初期状態 / 再生停止)
//   mode-full… フルプレイヤー表示 (アンビエント背景付き、背後スクロール停止)
//   mode-mini… ミニバー表示 (一覧の上に表示、再生は継続)
//
// 使い方:
//   const player = createPlayerUI(rootEl, { onBack: () => {} });
//   player.setAlbums(albums);
//   player.loadAlbum(cdId, null, true);   // 読み込み + 再生開始 (フル表示にはしない)
//   player.show();   // フル表示
//   player.hide();   // ミニ表示へ (現在トラックがあれば再生継続)
//   player.close();  // 完全停止

function createPlayerUI(rootEl, opts) {
    const onBack = (opts && opts.onBack) || (() => {});

    rootEl.classList.add("player-root", "hidden");
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
    let currentCd = null;
    let ambientFlip = false;

    function formatTime(sec) {
        if (!isFinite(sec) || sec < 0) sec = 0;
        const m = Math.floor(sec / 60);
        const s = Math.floor(sec % 60);
        return `${m}:${String(s).padStart(2, "0")}`;
    }

    function coverUrl(cd) {
        return cd && cd.cover_url ? `/images/${cd.cover_url}` : null;
    }

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

    function setCover(cd) {
        const url = coverUrl(cd);
        if (url) {
            el["cover-img"].src = url;
            el["cover-img"].alt = cd.title || "";
            el["cover-img"].style.display = "block";
            el["cover-fallback"].style.display = "none";
            el["mini-cover"].src = url;
            el["mini-cover"].style.display = "block";
            el["mini-cover-fallback"].style.display = "none";
        } else {
            el["cover-img"].removeAttribute("src");
            el["cover-img"].style.display = "none";
            el["cover-fallback"].style.display = "flex";
            el["mini-cover"].removeAttribute("src");
            el["mini-cover"].style.display = "none";
            el["mini-cover-fallback"].style.display = "flex";
        }
        setAmbient(url);
    }

    function renderTracklist(cd) {
        const tracks = engine.allTracks || [];
        const multiDisc = (cd.disc_count || 1) > 1 || tracks.some((t) => (t.disc_number || 1) > 1);
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
            <li class="player-track${playable ? "" : " player-track--disabled"}" data-track-id="${t.id}">
                <span class="player-track-num">
                    <span class="player-track-num-text">${num}</span>
                    <span class="player-eq" aria-hidden="true"><i></i><i></i><i></i></span>
                </span>
                <span class="player-track-title-cell">${escapeHtml(t.title)}</span>
                <span class="player-track-dur">${t.duration ? escapeHtml(t.duration) : (playable ? "" : "—")}</span>
            </li>`;
        }
        el.tracklist.innerHTML = html || '<li class="player-track-empty">トラックがありません</li>';
        const playableCount = tracks.filter((t) => t.file_hash).length;
        el["tracklist-count"].textContent = `${playableCount} 曲`;
    }

    function highlightCurrentTrack() {
        const cur = engine.current();
        el.tracklist.querySelectorAll(".player-track").forEach((li) => {
            const id = parseInt(li.dataset.trackId, 10);
            li.classList.toggle("player-track--current", !!cur && cur.id === id);
        });
    }

    function updateNowPlaying() {
        const t = engine.current();
        const title = t ? t.title : "再生できるトラックがありません";
        el["track-title"].textContent = title;
        el["album-title"].textContent = currentCd ? currentCd.title : "—";
        el["artist-name"].textContent = currentCd ? (currentCd.artist || "") : "";
        el["mini-title"].textContent = title;
        el["mini-artist"].textContent = currentCd ? (currentCd.artist || currentCd.title) : "";
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

    function updateProgress(pos) {
        const dur = engine.getDuration();
        const frac = dur > 0 ? Math.min(pos / dur, 1) : 0;
        const pct = (frac * 100).toFixed(3) + "%";
        el["progress-fill"].style.width = pct;
        el["progress-thumb"].style.left = pct;
        el["time-current"].textContent = formatTime(pos);
        el["mini-fill"].style.width = pct;
    }

    function updateDuration(dur) {
        el["time-total"].textContent = formatTime(dur);
    }

    function setPlayState(playing) {
        const icon = playing ? "pause" : "play_arrow";
        el["play-icon"].textContent = icon;
        el["mini-play-icon"].textContent = icon;
        el.cover.classList.toggle("is-playing", playing);
        el["status-dot"].classList.toggle("is-playing", playing);
        el["status-label"].textContent = playing ? "NOW PLAYING" : "PAUSED";
        el.tracklist.classList.toggle("is-playing", playing);
        rootEl.classList.toggle("is-playing", playing);
        if ("mediaSession" in navigator) {
            navigator.mediaSession.playbackState = playing ? "playing" : "paused";
        }
    }

    function currentAlbumIndex() {
        if (!currentCd) return -1;
        return albums.findIndex((c) => c.id === currentCd.id);
    }

    function goToAlbum(delta) {
        if (albums.length === 0) return;
        let idx = currentAlbumIndex();
        if (idx < 0) idx = delta > 0 ? -1 : 0;
        const next = (idx + delta + albums.length) % albums.length;
        api.loadAlbum(albums[next].id, null, true);
    }

    // ---------- 状態遷移 ----------
    function setMode(mode) {
        rootEl.classList.remove("hidden", "mode-full", "mode-mini");
        document.body.classList.remove("player-active", "player-mini-active");
        if (mode === "full") {
            rootEl.classList.add("mode-full");
            document.body.classList.add("player-active");
        } else if (mode === "mini") {
            rootEl.classList.add("mode-mini");
            document.body.classList.add("player-mini-active");
        } else {
            rootEl.classList.add("hidden");
        }
    }

    // ---------- シーク ----------
    function seekFromEvent(e) {
        const rect = el["progress-bar"].getBoundingClientRect();
        const frac = (e.clientX - rect.left) / rect.width;
        engine.seek(frac);
    }
    let dragging = false;
    el["progress-bar"].addEventListener("pointerdown", (e) => {
        dragging = true;
        el["progress-bar"].setPointerCapture(e.pointerId);
        seekFromEvent(e);
    });
    el["progress-bar"].addEventListener("pointermove", (e) => { if (dragging) seekFromEvent(e); });
    el["progress-bar"].addEventListener("pointerup", () => { dragging = false; });
    el["progress-bar"].addEventListener("keydown", (e) => {
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
    el.volume.addEventListener("input", () => {
        engine.setVolume(el.volume.value / 100);
        updateVolumeIcon();
    });

    // ---------- エンジンイベント ----------
    engine.on("trackchange", updateNowPlaying);
    engine.on("time", updateProgress);
    engine.on("duration", updateDuration);
    engine.on("playstate", setPlayState);
    engine.on("error", () => { setTimeout(() => engine.next(true), 300); });

    el.tracklist.addEventListener("click", (e) => {
        const li = e.target.closest(".player-track");
        if (!li || li.classList.contains("player-track--disabled")) return;
        engine.playTrackById(parseInt(li.dataset.trackId, 10));
    });

    rootEl.addEventListener("click", (e) => {
        const btn = e.target.closest("[data-act]");
        if (!btn) return;
        const act = btn.dataset.act;
        if (act === "back") { onBack(); api.hide(); }
        else if (act === "expand") api.show();
        else if (act === "close") api.close();
        else if (act === "play") engine.toggle();
        else if (act === "next") engine.next(false);
        else if (act === "prev") engine.prev();
        else if (act === "next-album") goToAlbum(1);
        else if (act === "prev-album") goToAlbum(-1);
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
        loadAlbum(cdId, startTrackId, autoplay) {
            const cd = albums.find((c) => c.id === cdId);
            if (!cd) return;
            currentCd = cd;
            setCover(cd);
            engine.loadTracks(cd.tracks || [], startTrackId);
            renderTracklist(cd);
            updateNowPlaying();
            updateProgress(0);
            updateDuration(0);
            if (autoplay) engine.play();
        },
        currentCd() { return currentCd; },
        show() {
            if (!engine.current()) return;
            setMode("full");
        },
        // 現在トラックがあればミニ表示へ (再生継続)、なければ完全停止
        hide() {
            if (engine.current()) setMode("mini");
            else api.close();
        },
        close() {
            engine.pause();
            setMode("closed");
        },
        get visible() { return rootEl.classList.contains("mode-full"); },
        get miniVisible() { return rootEl.classList.contains("mode-mini"); },
    };

    return api;
}
