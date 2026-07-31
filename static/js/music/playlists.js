let allPlaylists = [];

function getPlaylists() {
    return allPlaylists;
}

async function loadPlaylists() {
    try {
        const res = await fetch("/api/playlists");
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        allPlaylists = await res.json();
    } catch (err) {
        console.error("loadPlaylists failed:", err);
        allPlaylists = [];
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
    <div class="music-album music-playlist" data-playlist-id="${playlist.id}" tabindex="0" role="button" aria-label="${escapeAttr(playlist.name)} を再生">
        <div class="music-album-coverwrap">
            ${cover}
            <span class="music-album-badge music-album-badge--playlist">PL</span>
            <div class="music-album-play" data-album-action="play">
                <span class="music-album-play-btn"><span class="material-icons">play_arrow</span></span>
            </div>
            <button type="button" class="music-playlist-edit" data-playlist-action="edit" aria-label="${escapeAttr(playlist.name)}を編集" title="編集">
                <span class="material-icons">edit</span>
            </button>
        </div>
        <div class="music-album-name">${escapeHtml(playlist.name)}</div>
        <div class="music-album-artist">プレイリスト</div>
        <div class="music-album-meta">${trackCount} 曲</div>
    </div>`;
}

function playlistTrackRows() {
    const groups = new Map();
    for (const cd of allAlbums || []) {
        const tracks = (cd.tracks || []).filter((track) => track.file_hash);
        if (tracks.length === 0) continue;
        groups.set(cd.id, { cd, tracks });
    }
    return [...groups.values()];
}

function renderPlaylistTrackPicker(selectedIds) {
    const picker = document.getElementById("playlist-track-picker");
    const selected = new Set((selectedIds || []).map(Number));
    const groups = playlistTrackRows();
    if (groups.length === 0) {
        picker.innerHTML = "<p class=\"playlist-empty-tracks\">追加できる音声トラックがありません</p>";
        return;
    }
    picker.innerHTML = groups.map(({ cd, tracks }) => `
        <section class="playlist-track-group">
            <strong class="playlist-track-group-title">${escapeHtml(cd.title)}</strong>
            ${tracks.map((track) => `
                <label class="playlist-track-option">
                    <input type="checkbox" value="${track.id}"${selected.has(track.id) ? " checked" : ""}>
                    <span>${escapeHtml(track.title)}</span>
                    <small>${escapeHtml(cd.artist || (cd.media_type === "audiobook" ? "オーディオブック" : ""))}</small>
                </label>`).join("")}
        </section>`).join("");
}

function renderPlaylistCoverOptions(selectedId) {
    const select = document.getElementById("playlist-cover-cd");
    const options = ["<option value=\"\">カバーなし</option>"];
    for (const cd of allAlbums || []) {
        const label = cd.artist ? `${cd.title} · ${cd.artist}` : cd.title;
        options.push(`<option value="${cd.id}"${cd.id === selectedId ? " selected" : ""}>${escapeHtml(label)}</option>`);
    }
    select.innerHTML = options.join("");
}

function setPlaylistFormStatus(message, isError = false) {
    const status = document.getElementById("playlist-form-status");
    status.textContent = message || "";
    status.classList.toggle("is-error", isError);
}

function openPlaylistEditor(id = null) {
    const modal = document.getElementById("playlist-modal");
    const title = document.getElementById("playlist-dialog-title");
    const name = document.getElementById("playlist-name");
    const description = document.getElementById("playlist-description");
    const deleteButton = document.getElementById("playlist-delete-button");
    const playlist = id == null ? null : allPlaylists.find((item) => item.id === Number(id));
    modal.dataset.playlistId = playlist ? String(playlist.id) : "";
    title.textContent = playlist ? "プレイリストを編集" : "プレイリストを作成";
    name.value = playlist ? playlist.name : "";
    description.value = playlist ? (playlist.description || "") : "";
    deleteButton.hidden = !playlist;
    renderPlaylistCoverOptions(playlist ? playlist.cover_cd_id : null);
    renderPlaylistTrackPicker(playlist ? playlist.tracks.map((entry) => entry.track.id) : []);
    setPlaylistFormStatus("");
    modal.hidden = false;
    name.focus();
}

function closePlaylistEditor() {
    document.getElementById("playlist-modal").hidden = true;
}

async function playlistRequest(url, options) {
    const res = await fetch(url, options);
    let data = null;
    try { data = await res.json(); } catch {}
    if (!res.ok) throw new Error(data && data.error ? data.error : `HTTP ${res.status}`);
    return data;
}

async function savePlaylist(event) {
    event.preventDefault();
    const modal = document.getElementById("playlist-modal");
    const id = parseInt(modal.dataset.playlistId, 10);
    const name = document.getElementById("playlist-name").value.trim();
    const description = document.getElementById("playlist-description").value.trim() || null;
    const coverValue = document.getElementById("playlist-cover-cd").value;
    const trackIds = [...document.querySelectorAll("#playlist-track-picker input:checked")]
        .map((input) => parseInt(input.value, 10))
        .filter(Number.isFinite);
    const saveButton = document.querySelector("#playlist-form button[type=submit]");
    saveButton.disabled = true;
    setPlaylistFormStatus("保存中...");
    try {
        const payload = {
            name,
            description,
            cover_cd_id: coverValue ? parseInt(coverValue, 10) : null,
        };
        let playlist;
        if (Number.isFinite(id)) {
            playlist = await playlistRequest(`/api/playlists/${id}`, {
                method: "PUT",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(payload),
            });
        } else {
            playlist = await playlistRequest("/api/playlists", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(payload),
            });
        }
        await playlistRequest(`/api/playlists/${playlist.id}/tracks`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ track_ids: trackIds }),
        });
        await loadPlaylists();
        closePlaylistEditor();
        if (typeof renderGrid === "function") renderGrid();
    } catch (err) {
        setPlaylistFormStatus(err.message || "保存に失敗しました", true);
    } finally {
        saveButton.disabled = false;
    }
}

async function deletePlaylist() {
    const modal = document.getElementById("playlist-modal");
    const id = parseInt(modal.dataset.playlistId, 10);
    const playlist = allPlaylists.find((item) => item.id === id);
    if (!playlist || !await showConfirm({ message: `プレイリスト「${playlist.name}」を削除しますか？`, okLabel: "削除" })) return;
    try {
        await playlistRequest(`/api/playlists/${id}`, { method: "DELETE" });
        await loadPlaylists();
        closePlaylistEditor();
        if (typeof renderGrid === "function") renderGrid();
    } catch (err) {
        setPlaylistFormStatus(err.message || "削除に失敗しました", true);
    }
}

function openPlaylist(id) {
    const playlist = allPlaylists.find((item) => item.id === Number(id));
    if (!playlist) return;
    const entries = (playlist.tracks || [])
        .filter((entry) => entry.track && entry.track.file_hash && entry.cd)
        .map((entry) => ({
            track: entry.track,
            album: { ...entry.cd, tracks: [entry.track] },
        }));
    if (entries.length === 0) {
        openPlaylistEditor(playlist.id);
        return;
    }
    if (window.musicPlayer) window.musicPlayer.openQueue(entries, 0);
}

document.getElementById("playlist-create-button").addEventListener("click", () => openPlaylistEditor());
document.getElementById("playlist-close-button").addEventListener("click", closePlaylistEditor);
document.getElementById("playlist-cancel-button").addEventListener("click", closePlaylistEditor);
document.getElementById("playlist-form").addEventListener("submit", savePlaylist);
document.getElementById("playlist-delete-button").addEventListener("click", deletePlaylist);
document.getElementById("playlist-modal").addEventListener("click", (event) => {
    if (event.target === event.currentTarget) closePlaylistEditor();
});
