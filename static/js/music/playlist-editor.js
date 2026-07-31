let playlistEditorState = null;
let playlistEditorDragIndex = null;

function editorCdKey(entry) {
    return entry && entry.cd && entry.cd.id != null ? String(entry.cd.id) : "unknown";
}

function editorCdBounds(entries, index) {
    if (index < 0 || index >= entries.length) return null;
    const key = editorCdKey(entries[index]);
    let start = index;
    let end = index;
    while (start > 0 && editorCdKey(entries[start - 1]) === key) start -= 1;
    while (end + 1 < entries.length && editorCdKey(entries[end + 1]) === key) end += 1;
    return { start, end };
}

function setPlaylistEditorStatus(modal, message, isError = false) {
    const status = modal.querySelector("[data-playlist-editor-status]");
    status.textContent = message || "";
    status.classList.toggle("is-error", isError);
}

function renderPlaylistEditorCovers(modal, selectedId) {
    const select = modal.querySelector("[data-playlist-editor-cover]");
    const options = ['<option value="">カバーなし</option>'];
    for (const cd of window.musicAlbums || []) {
        const artist = getCdArtistName(cd);
        const label = artist ? `${cd.title} · ${artist}` : cd.title;
        options.push(`<option value="${cd.id}"${cd.id === selectedId ? " selected" : ""}>${escapeHtml(label)}</option>`);
    }
    select.innerHTML = options.join("");
}

function renderPlaylistEditorEntries(modal) {
    const list = modal.querySelector("[data-playlist-editor-entries]");
    const state = playlistEditorState;
    modal.querySelector("[data-playlist-editor-count]").textContent = `${state ? state.entries.length : 0} 曲`;
    if (!state || state.entries.length === 0) {
        list.innerHTML = '<li class="playlist-editor-empty">トラックがありません。再生詳細画面から CD または曲を追加できます。</li>';
        return;
    }

    let lastCdKey = null;
    const html = [];
    state.entries.forEach((entry, index) => {
        const cd = entry.cd || {};
        const key = editorCdKey(entry);
        if (key !== lastCdKey) {
            const bounds = editorCdBounds(state.entries, index);
            const cdCount = bounds.end - bounds.start + 1;
            html.push(`
                <li class="playlist-editor-cd" data-playlist-editor-cd="${index}">
                    <div class="playlist-editor-cd-copy">
                        <strong>${escapeHtml(cd.title || "不明なCD")}</strong>
                        <small>${escapeHtml(getCdArtistName(cd))} · ${cdCount}曲</small>
                    </div>
                    <div class="playlist-editor-cd-actions">
                        <button type="button" class="playlist-editor-icon-button" data-playlist-editor-cd-move="-1" data-index="${index}" ${bounds.start === 0 ? "disabled" : ""} aria-label="${escapeAttr(cd.title || "CD")}を上へ" title="CDを上へ">
                            <span class="material-icons">keyboard_arrow_up</span>
                        </button>
                        <button type="button" class="playlist-editor-icon-button" data-playlist-editor-cd-move="1" data-index="${index}" ${bounds.end === state.entries.length - 1 ? "disabled" : ""} aria-label="${escapeAttr(cd.title || "CD")}を下へ" title="CDを下へ">
                            <span class="material-icons">keyboard_arrow_down</span>
                        </button>
                    </div>
                </li>`);
            lastCdKey = key;
        }

        const track = entry.track || {};
        html.push(`
            <li class="playlist-editor-track" data-playlist-editor-row data-index="${index}" draggable="true">
                <span class="playlist-editor-drag" aria-hidden="true"><span class="material-icons">drag_indicator</span></span>
                <span class="playlist-editor-track-number">${String(index + 1).padStart(2, "0")}</span>
                <span class="playlist-editor-track-copy">
                    <strong>${escapeHtml(track.title || "無題のトラック")}</strong>
                    <small>${track.duration ? escapeHtml(track.duration) : ""}${track.file_hash ? "" : " · 音声ファイルなし"}</small>
                </span>
                <div class="playlist-editor-track-actions">
                    <button type="button" class="playlist-editor-icon-button" data-playlist-editor-track-move="-1" data-index="${index}" ${index === 0 ? "disabled" : ""} aria-label="${escapeAttr(track.title || "トラック")}を上へ" title="トラックを上へ">
                        <span class="material-icons">keyboard_arrow_up</span>
                    </button>
                    <button type="button" class="playlist-editor-icon-button" data-playlist-editor-track-move="1" data-index="${index}" ${index === state.entries.length - 1 ? "disabled" : ""} aria-label="${escapeAttr(track.title || "トラック")}を下へ" title="トラックを下へ">
                        <span class="material-icons">keyboard_arrow_down</span>
                    </button>
                    <button type="button" class="playlist-editor-icon-button playlist-editor-remove" data-playlist-editor-remove data-index="${index}" aria-label="${escapeAttr(track.title || "トラック")}を削除" title="トラックを削除">
                        <span class="material-icons">close</span>
                    </button>
                </div>
            </li>`);
    });
    list.innerHTML = html.join("");
}

function movePlaylistEditorTrack(index, delta, modal) {
    const entries = playlistEditorState && playlistEditorState.entries;
    const target = index + delta;
    if (!entries || index < 0 || target < 0 || target >= entries.length) return;
    [entries[index], entries[target]] = [entries[target], entries[index]];
    renderPlaylistEditorEntries(modal);
}

function movePlaylistEditorCd(index, delta, modal) {
    const entries = playlistEditorState && playlistEditorState.entries;
    const bounds = entries && editorCdBounds(entries, index);
    if (!bounds) return;

    const blockLength = bounds.end - bounds.start + 1;
    if (delta < 0 && bounds.start > 0) {
        const previous = editorCdBounds(entries, bounds.start - 1);
        const block = entries.splice(bounds.start, blockLength);
        entries.splice(previous.start, 0, ...block);
    } else if (delta > 0 && bounds.end < entries.length - 1) {
        const next = editorCdBounds(entries, bounds.end + 1);
        const block = entries.splice(bounds.start, blockLength);
        const nextLength = next.end - next.start + 1;
        entries.splice(bounds.start + nextLength, 0, ...block);
    } else {
        return;
    }
    renderPlaylistEditorEntries(modal);
}

function removePlaylistEditorTrack(index, modal) {
    if (!playlistEditorState || index < 0 || index >= playlistEditorState.entries.length) return;
    playlistEditorState.entries.splice(index, 1);
    renderPlaylistEditorEntries(modal);
}

function ensurePlaylistEditor() {
    let modal = document.getElementById("playlist-editor-modal");
    if (modal) return modal;

    modal = document.createElement("div");
    modal.id = "playlist-editor-modal";
    modal.className = "playlist-modal playlist-editor-modal";
    modal.hidden = true;
    modal.innerHTML = `
        <section class="playlist-dialog playlist-editor-dialog" role="dialog" aria-modal="true" aria-labelledby="playlist-editor-title">
            <header class="playlist-dialog-header">
                <div>
                    <span class="playlist-picker-kicker">PLAYLIST EDITOR</span>
                    <h2 id="playlist-editor-title">プレイリストを編集</h2>
                </div>
                <button type="button" class="playlist-dialog-close" data-playlist-editor-close aria-label="閉じる">
                    <span class="material-icons">close</span>
                </button>
            </header>
            <form class="playlist-editor-form" data-playlist-editor-form>
                <div class="playlist-editor-fields">
                    <label class="playlist-field">
                        <span>名前</span>
                        <input class="playlist-input" data-playlist-editor-name type="text" maxlength="200" required>
                    </label>
                    <label class="playlist-field">
                        <span>説明</span>
                        <textarea class="playlist-input playlist-textarea" data-playlist-editor-description rows="2" maxlength="1000"></textarea>
                    </label>
                    <label class="playlist-field">
                        <span>カバーに使うCD</span>
                        <select class="playlist-input" data-playlist-editor-cover></select>
                    </label>
                </div>
                <div class="playlist-editor-order-heading">
                    <div>
                        <strong>再生順</strong>
                        <small>トラックは矢印またはドラッグで移動、CD見出しの矢印でCD単位に移動できます。</small>
                    </div>
                    <span data-playlist-editor-count></span>
                </div>
                <ul class="playlist-editor-entries" data-playlist-editor-entries></ul>
                <p class="playlist-form-status" data-playlist-editor-status role="status"></p>
                <div class="playlist-dialog-actions playlist-editor-actions">
                    <span></span>
                    <button type="button" class="playlist-cancel-button" data-playlist-editor-close>キャンセル</button>
                    <button type="submit" class="playlist-save-button" data-playlist-editor-save>
                        <span class="material-icons">save</span>保存
                    </button>
                </div>
            </form>
        </section>`;
    document.body.appendChild(modal);

    const close = () => {
        modal.hidden = true;
        playlistEditorState = null;
        playlistEditorDragIndex = null;
    };

    modal.addEventListener("click", (event) => {
        if (event.target === modal || event.target.closest("[data-playlist-editor-close]")) {
            close();
            return;
        }
        const trackMove = event.target.closest("[data-playlist-editor-track-move]");
        if (trackMove && !trackMove.disabled) {
            movePlaylistEditorTrack(Number(trackMove.dataset.index), Number(trackMove.dataset.playlistEditorTrackMove), modal);
            return;
        }
        const cdMove = event.target.closest("[data-playlist-editor-cd-move]");
        if (cdMove && !cdMove.disabled) {
            movePlaylistEditorCd(Number(cdMove.dataset.index), Number(cdMove.dataset.playlistEditorCdMove), modal);
            return;
        }
        const remove = event.target.closest("[data-playlist-editor-remove]");
        if (remove) removePlaylistEditorTrack(Number(remove.dataset.index), modal);
    });

    modal.addEventListener("dragstart", (event) => {
        const row = event.target.closest("[data-playlist-editor-row]");
        if (!row) return;
        playlistEditorDragIndex = Number(row.dataset.index);
        row.classList.add("is-dragging");
        event.dataTransfer.effectAllowed = "move";
        event.dataTransfer.setData("text/plain", String(playlistEditorDragIndex));
    });
    modal.addEventListener("dragover", (event) => {
        const row = event.target.closest("[data-playlist-editor-row]");
        if (!row || playlistEditorDragIndex == null) return;
        event.preventDefault();
        row.classList.add("is-drag-over");
        event.dataTransfer.dropEffect = "move";
    });
    modal.addEventListener("dragleave", (event) => {
        const row = event.target.closest("[data-playlist-editor-row]");
        if (row && !row.contains(event.relatedTarget)) row.classList.remove("is-drag-over");
    });
    modal.addEventListener("drop", (event) => {
        const row = event.target.closest("[data-playlist-editor-row]");
        if (!row || playlistEditorDragIndex == null || !playlistEditorState) return;
        event.preventDefault();
        const from = playlistEditorDragIndex;
        const to = Number(row.dataset.index);
        if (from !== to) {
            const [entry] = playlistEditorState.entries.splice(from, 1);
            playlistEditorState.entries.splice(to, 0, entry);
        }
        playlistEditorDragIndex = null;
        renderPlaylistEditorEntries(modal);
    });
    modal.addEventListener("dragend", () => {
        playlistEditorDragIndex = null;
        modal.querySelectorAll(".is-dragging, .is-drag-over").forEach((row) => {
            row.classList.remove("is-dragging", "is-drag-over");
        });
    });
    modal.querySelector("[data-playlist-editor-form]").addEventListener("submit", async (event) => {
        event.preventDefault();
        if (!playlistEditorState) return;
        const form = event.currentTarget;
        const name = form.querySelector("[data-playlist-editor-name]").value.trim();
        const description = form.querySelector("[data-playlist-editor-description]").value.trim();
        const coverValue = form.querySelector("[data-playlist-editor-cover]").value;
        const save = form.querySelector("[data-playlist-editor-save]");
        if (!name) return;

        save.disabled = true;
        setPlaylistEditorStatus(modal, "保存中...");
        try {
            await playlistRequest(`/api/playlists/${playlistEditorState.id}`, {
                method: "PUT",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    name,
                    description: description || null,
                    cover_cd_id: coverValue ? Number(coverValue) : null,
                    track_ids: playlistEditorState.entries.map((entry) => entry.track.id),
                }),
            });
            await loadPlaylists();
            if (typeof renderGrid === "function") renderGrid();
            close();
        } catch (err) {
            setPlaylistEditorStatus(modal, err.message || "プレイリストの保存に失敗しました", true);
        } finally {
            save.disabled = false;
        }
    });
    document.addEventListener("keydown", (event) => {
        if (event.key === "Escape" && !modal.hidden) close();
    });

    return modal;
}

function openPlaylistEditor(id) {
    const playlist = allPlaylists.find((item) => item.id === Number(id));
    if (!playlist) return;

    const modal = ensurePlaylistEditor();
    playlistEditorState = {
        id: playlist.id,
        entries: (playlist.tracks || []).map((entry) => ({ track: entry.track, cd: entry.cd })),
    };
    modal.querySelector("[data-playlist-editor-name]").value = playlist.name || "";
    modal.querySelector("[data-playlist-editor-description]").value = playlist.description || "";
    renderPlaylistEditorCovers(modal, playlist.cover_cd_id == null ? null : Number(playlist.cover_cd_id));
    modal.querySelector("[data-playlist-editor-count]").textContent = `${playlistEditorState.entries.length} 曲`;
    setPlaylistEditorStatus(modal, "");
    renderPlaylistEditorEntries(modal);
    modal.hidden = false;
    modal.querySelector("[data-playlist-editor-name]").focus();
}
