function renderTracksHtml(tracks, editType, parentId) {
    if (tracks.length === 0) {
        return "<p class='series-empty'>トラックなし</p>";
    }

    const discGroups = {};
    for (const t of tracks) {
        const d = t.disc_number || 1;
        if (!discGroups[d]) discGroups[d] = [];
        discGroups[d].push(t);
    }

    let html = "";
    const discKeys = Object.keys(discGroups).sort((a, b) => a - b);
    for (const d of discKeys) {
        if (discKeys.length > 1) html += `<div class="detail-tracks-disc">Disc ${d}</div>`;
        html += discGroups[d].map((t) => `
            <div class="edit-track-row">
                <span class="edit-track-num">${String(t.track_number).padStart(2, "0")}</span>
                <span class="edit-track-title-text">${escapeHtml(t.title)}</span>
                ${t.duration ? `<span class="edit-track-dur">${escapeHtml(t.duration)}</span>` : ""}
                <div class="edit-track-audio">
                    ${t.file_hash
                        ? `<span class="edit-track-file">${escapeHtml(t.file_name || t.file_hash)}</span>
                           <button class="btn btn-xs btn-ghost" onclick="playAudio('/audio/${t.file_hash}','${escapeAttr(t.title)}')" aria-label="再生">
                               <span class="material-icons" aria-hidden="true">play_arrow</span>
                           </button>
                           <button class="btn btn-xs btn-outline-danger" onclick="deleteTrackAudio('${editType}',${parentId},${t.id})">削除</button>`
                        : `<label class="btn btn-xs btn-outline-success" style="cursor:pointer">
                               アップロード
                               <input type="file" accept="audio/*" hidden onchange="uploadTrackAudio('${editType}',${parentId},${t.id},this)">
                           </label>`}
                </div>
            </div>
        `).join("");
    }
    return html;
}

async function loadAndRenderTracks(bookId) {
    const list = document.getElementById("edit-tracks-list");
    if (!list) return;

    try {
        const res = await fetch(`/api/books/${bookId}/tracks`);
        if (!res.ok) { list.innerHTML = "<p class='series-empty'>トラックなし</p>"; return; }
        const tracks = await res.json();
        list.innerHTML = renderTracksHtml(tracks, "book", bookId);
    } catch {
        list.innerHTML = "<p class='series-empty'>トラック読み込みエラー</p>";
    }
}

async function loadAndRenderCdTracks(cdId) {
    const list = document.getElementById("edit-tracks-list");
    if (!list) return;

    try {
        const res = await fetch(`/api/cds/${cdId}/tracks`);
        if (!res.ok) { list.innerHTML = "<p class='series-empty'>トラックなし</p>"; return; }
        const tracks = await res.json();
        list.innerHTML = renderTracksHtml(tracks, "cd", cdId);
    } catch {
        list.innerHTML = "<p class='series-empty'>トラック読み込みエラー</p>";
    }
}

async function uploadTrackAudio(editType, parentId, trackId, input) {
    const file = input.files[0];
    if (!file) return;
    const fd = new FormData();
    fd.append("audio", file);
    try {
        const url = editType === "cd"
            ? `/api/cds/${parentId}/tracks/${trackId}/audio`
            : `/api/books/${parentId}/tracks/${trackId}/audio`;
        const res = await fetch(url, { method: "POST", body: fd });
        if (res.ok) {
            if (editType === "cd") loadAndRenderCdTracks(parentId);
            else loadAndRenderTracks(parentId);
        }
    } catch {}
}

async function deleteTrackAudio(editType, parentId, trackId) {
    if (!await showConfirm({ message: "音声ファイルを削除しますか？", okLabel: "削除" })) return;
    try {
        const url = editType === "cd"
            ? `/api/cds/${parentId}/tracks/${trackId}/audio`
            : `/api/books/${parentId}/tracks/${trackId}/audio`;
        const res = await fetch(url, { method: "DELETE" });
        if (res.ok) {
            if (editType === "cd") loadAndRenderCdTracks(parentId);
            else loadAndRenderTracks(parentId);
        }
    } catch {}
}