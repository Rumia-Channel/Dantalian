let allPlaylists = [];
let playlistLoadError = null;

function getPlaylists() {
    return allPlaylists;
}

async function loadPlaylists() {
    try {
        const res = await fetch("/api/playlists");
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        allPlaylists = await res.json();
        playlistLoadError = null;
        return true;
    } catch (err) {
        console.error("loadPlaylists failed:", err);
        allPlaylists = [];
        playlistLoadError = err instanceof Error ? err.message : String(err);
        return false;
    }
}

function playlistCoverUrl(playlist) {
    return playlist && playlist.cover_url ? `/images/${playlist.cover_url}` : null;
}

function renderPlaylistCard(playlist) {
    const coverUrl = playlistCoverUrl(playlist);
    const cover = coverUrl
        ? `<img class="music-album-cover" src="${escapeAttr(coverUrl)}" alt="${escapeAttr(playlist.name)}" loading="lazy">`
        : "<div class=\"music-album-coverfallback\"><span class=\"material-icons\">queue_music</span></div>";
    const trackCount = (playlist.tracks || []).filter((entry) => entry.track && entry.track.file_hash).length;
    return `
    <div class="music-album music-playlist" data-playlist-id="${playlist.id}" tabindex="0" role="button" aria-label="${escapeAttr(playlist.name)} を表示">
        <div class="music-album-coverwrap">
            ${cover}
            <span class="music-album-badge music-album-badge--playlist">PL</span>
            <div class="music-album-play">
                <button type="button" class="music-album-play-btn" data-album-action="play" aria-label="${escapeAttr(playlist.name)}を再生">
                    <span class="material-icons">play_arrow</span>
                </button>
            </div>
            <button type="button" class="music-playlist-edit" data-playlist-action="edit" aria-label="${escapeAttr(playlist.name)}を編集" title="プレイリストを編集">
                <span class="material-icons">edit</span>
            </button>
            <button type="button" class="music-playlist-delete" data-playlist-action="delete" aria-label="${escapeAttr(playlist.name)}を削除" title="プレイリストを削除">
                <span class="material-icons">delete</span>
            </button>
        </div>
        <div class="music-album-name">${escapeHtml(playlist.name)}</div>
        <div class="music-album-artist">プレイリスト</div>
        <div class="music-album-meta">${trackCount} 曲</div>
    </div>`;
}

async function playlistRequest(url, options) {
    const res = await fetch(url, options);
    let data = null;
    try { data = await res.json(); } catch {}
    if (!res.ok) throw new Error(data && data.error ? data.error : `HTTP ${res.status}`);
    return data;
}

async function deletePlaylistById(id) {
    await playlistRequest(`/api/playlists/${id}`, { method: "DELETE" });
    await loadPlaylists();
    if (typeof renderGrid === "function") renderGrid();
}

async function removePlaylistTracksById(playlistId, trackIds) {
    const ids = [...new Set((trackIds || []).map(Number).filter(Number.isFinite))];
    const playlist = allPlaylists.find((item) => item.id === Number(playlistId));
    if (!playlist) throw new Error("プレイリストが見つかりません");
    const removeSet = new Set(ids);
    const trackIdsAfter = (playlist.tracks || [])
        .map((entry) => Number(entry.track?.id))
        .filter((trackId) => Number.isFinite(trackId) && !removeSet.has(trackId));
    await playlistRequest(`/api/playlists/${playlistId}/tracks`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ track_ids: trackIdsAfter }),
    });
    await loadPlaylists();
    if (typeof renderGrid === "function") renderGrid();
}

async function confirmDeletePlaylist(id) {
    const playlist = allPlaylists.find((item) => item.id === Number(id));
    if (!playlist) return;
    if (typeof showConfirm === "function") {
        const confirmed = await showConfirm({
            message: `プレイリスト「${playlist.name}」を削除しますか？`,
            okLabel: "削除",
        });
        if (!confirmed) return;
    } else if (!window.confirm(`プレイリスト「${playlist.name}」を削除しますか？`)) {
        return;
    }
    await deletePlaylistById(playlist.id);
}

function playlistTrackIdsFromModal(modal) {
    try {
        const ids = JSON.parse(modal.dataset.trackIds || "[]");
        return [...new Set(ids.map(Number).filter(Number.isFinite))];
    } catch {
        return [];
    }
}

function setPlaylistPickerStatus(modal, message, isError = false) {
    const status = modal.querySelector("[data-playlist-picker-status]");
    status.textContent = message || "";
    status.classList.toggle("is-error", isError);
}

function renderPlaylistPickerOptions(modal) {
    const options = modal.querySelector("[data-playlist-picker-options]");
    if (allPlaylists.length === 0) {
        options.innerHTML = '<p class="playlist-picker-empty">保存済みのプレイリストはありません</p>';
        return;
    }

    options.innerHTML = allPlaylists.map((playlist) => {
        const cover = playlistCoverUrl(playlist)
            ? `<img src="${escapeAttr(playlistCoverUrl(playlist))}" alt="" loading="lazy">`
            : '<span class="material-icons">queue_music</span>';
        const trackCount = (playlist.tracks || []).length;
        return `
        <button type="button" class="playlist-picker-option" data-playlist-id="${playlist.id}">
            <span class="playlist-picker-cover">${cover}</span>
            <span class="playlist-picker-copy">
                <strong>${escapeHtml(playlist.name)}</strong>
                <small>${trackCount} 曲</small>
            </span>
            <span class="material-icons">playlist_add</span>
        </button>`;
    }).join("");
}

function renderPlaylistPickerCovers(modal, selectedId) {
    const select = modal.querySelector("[data-playlist-picker-cover]");
    const options = ['<option value="">カバーなし</option>'];
    for (const cd of window.musicAlbums || []) {
        const label = cd.artist ? `${cd.title} · ${cd.artist}` : cd.title;
        options.push(`<option value="${cd.id}"${cd.id === selectedId ? " selected" : ""}>${escapeHtml(label)}</option>`);
    }
    select.innerHTML = options.join("");
}

function ensurePlaylistPicker() {
    let modal = document.getElementById("player-playlist-modal");
    if (modal) return modal;

    modal = document.createElement("div");
    modal.id = "player-playlist-modal";
    modal.className = "playlist-modal";
    modal.hidden = true;
    modal.innerHTML = `
        <section class="playlist-dialog playlist-picker-dialog" role="dialog" aria-modal="true" aria-labelledby="player-playlist-title">
            <header class="playlist-dialog-header">
                <div>
                    <span class="playlist-picker-kicker">PLAYER ACTION</span>
                    <h2 id="player-playlist-title">プレイリストに追加</h2>
                </div>
                <button type="button" class="playlist-dialog-close" data-playlist-picker-close aria-label="閉じる">
                    <span class="material-icons">close</span>
                </button>
            </header>
            <div class="playlist-picker-body">
                <p class="playlist-picker-target" data-playlist-picker-target></p>
                <div class="playlist-picker-options" data-playlist-picker-options></div>
                <form class="playlist-picker-new" data-playlist-picker-form>
                    <div class="playlist-picker-new-title">新規プレイリスト</div>
                    <label class="playlist-field">
                        <span>名前</span>
                        <input class="playlist-input" data-playlist-picker-name type="text" maxlength="200" required>
                    </label>
                    <label class="playlist-field">
                        <span>カバーに使うCD</span>
                        <select class="playlist-input" data-playlist-picker-cover></select>
                    </label>
                    <p class="playlist-form-status" data-playlist-picker-status role="status"></p>
                    <div class="playlist-picker-actions">
                        <button type="button" class="playlist-cancel-button" data-playlist-picker-close>キャンセル</button>
                        <button type="submit" class="playlist-save-button">作成して追加</button>
                    </div>
                </form>
            </div>
        </section>`;
    document.body.appendChild(modal);

    modal.addEventListener("click", async (event) => {
        if (event.target === modal || event.target.closest("[data-playlist-picker-close]")) {
            modal.hidden = true;
            return;
        }
        const button = event.target.closest("[data-playlist-id]");
        if (!button || button.disabled) return;
        const ids = playlistTrackIdsFromModal(modal);
        button.disabled = true;
        setPlaylistPickerStatus(modal, "追加中...");
        try {
            await addTrackIdsToPlaylist(Number(button.dataset.playlistId), ids);
            await loadPlaylists();
            modal.hidden = true;
            if (typeof renderGrid === "function") renderGrid();
        } catch (err) {
            button.disabled = false;
            setPlaylistPickerStatus(modal, err.message || "追加に失敗しました", true);
        }
    });

    modal.querySelector("[data-playlist-picker-form]").addEventListener("submit", async (event) => {
        event.preventDefault();
        const form = event.currentTarget;
        const name = form.querySelector("[data-playlist-picker-name]").value.trim();
        const coverValue = form.querySelector("[data-playlist-picker-cover]").value;
        const submit = form.querySelector("button[type=submit]");
        if (!name) return;
        submit.disabled = true;
        setPlaylistPickerStatus(modal, "作成中...");
        try {
            const playlist = await playlistRequest("/api/playlists", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    name,
                    description: null,
                    cover_cd_id: coverValue ? Number(coverValue) : null,
                }),
            });
            await addTrackIdsToPlaylist(playlist.id, playlistTrackIdsFromModal(modal));
            await loadPlaylists();
            modal.hidden = true;
            if (typeof renderGrid === "function") renderGrid();
        } catch (err) {
            setPlaylistPickerStatus(modal, err.message || "作成に失敗しました", true);
        } finally {
            submit.disabled = false;
        }
    });

    return modal;
}

async function addTrackIdsToPlaylist(playlistId, trackIds) {
    const playlist = allPlaylists.find((item) => item.id === Number(playlistId));
    const existing = (playlist?.tracks || [])
        .map((entry) => Number(entry.track?.id))
        .filter(Number.isFinite);
    const next = [...new Set([...existing, ...trackIds.map(Number).filter(Number.isFinite)])];
    await playlistRequest(`/api/playlists/${playlistId}/tracks`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ track_ids: next }),
    });
}

async function openPlaylistPicker({ trackIds, defaultCoverCdId = null } = {}) {
    const ids = [...new Set((trackIds || []).map(Number).filter(Number.isFinite))];
    if (ids.length === 0) return;
    if (allPlaylists.length === 0) await loadPlaylists();

    const modal = ensurePlaylistPicker();
    modal.dataset.trackIds = JSON.stringify(ids);
    modal.querySelector("[data-playlist-picker-target]").textContent = `${ids.length} 曲を追加します`;
    renderPlaylistPickerOptions(modal);
    renderPlaylistPickerCovers(modal, defaultCoverCdId == null ? null : Number(defaultCoverCdId));
    setPlaylistPickerStatus(modal, "");
    modal.hidden = false;
    modal.querySelector("[data-playlist-picker-name]").focus();
}

function openPlaylist(id, autoplay = false) {
    const playlist = allPlaylists.find((item) => item.id === Number(id));
    if (!playlist || !window.musicPlayer) return;
    const playlistEntries = (playlist.tracks || [])
        .filter((entry) => entry.track && entry.track.file_hash && entry.cd)
    if (playlistEntries.length === 0) {
        window.alert(`プレイリスト「${playlist.name}」には再生できる曲がありません。`);
        return;
    }

    const firstCd = playlistEntries[0].cd;
    const playlistAlbum = {
        ...firstCd,
        id: -Math.abs(Number(playlist.id)),
        title: playlist.name,
        artist: "プレイリスト",
        cover_url: playlist.cover_url || firstCd.cover_url,
        media_type: "playlist",
        playlist_id: playlist.id,
        playlistTrackEntries: playlistEntries,
        tracks: playlistEntries.map((entry) => entry.track),
    };
    const entries = playlistEntries.map((entry) => ({
        track: entry.track,
        album: playlistAlbum,
        sourceAlbum: entry.cd,
    }));
    if (autoplay) window.musicPlayer.openQueue(entries, 0);
    else window.musicPlayer.openQueuePreview(entries, 0);
}
