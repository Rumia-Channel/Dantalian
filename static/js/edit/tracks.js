function renderTracksHtml(tracks, editType, parentId) {
    if (tracks.length === 0) {
        if (editType === "cd") {
            return `<p class='series-empty'>トラックなし</p>
                <div style="margin-top:0.4rem">
                    <button class="btn btn-xs btn-outline-success" onclick="addTrackToDisc(${parentId},1,'${editType}')">+ トラック追加</button>
                </div>`;
        }
        return "<p class='series-empty'>トラックなし</p>";
    }

    const discGroups = {};
    for (const t of tracks) {
        const d = t.disc_number || 1;
        if (!discGroups[d]) discGroups[d] = [];
        discGroups[d].push(t);
    }

    const discKeys = Object.keys(discGroups).sort((a, b) => a - b);
    let html = "";

    for (const d of discKeys) {
        const discTracks = discGroups[d].sort((a, b) => a.track_number - b.track_number);
        if (discKeys.length > 1) {
            html += `<div class="detail-tracks-disc" style="display:flex;justify-content:space-between;align-items:center">
                <span>Disc ${d}</span>
                <button class="btn btn-xs btn-outline-success" onclick="addTrackToDisc(${parentId},${d},'${editType}')">+ トラック追加</button>
            </div>`;
        }
        html += discTracks.map((t, idx) => {
            const isFirst = idx === 0;
            const isLast = idx === discTracks.length - 1;
            const discId = parseInt(d, 10);
            const hasAudio = t.file_hash ? ' has-audio' : '';
            return `
                <div class="edit-track-row" data-track-id="${t.id}">
                    <span class="edit-track-num">${String(t.track_number).padStart(2, "0")}</span>
                    <input type="text" class="edit-track-title-input" value="${escapeAttr(t.title)}" data-track-id="${t.id}" onchange="saveTrackField(${parentId},${t.id},'title',this.value,'${editType}')">
                    <input type="text" class="edit-track-dur-input" value="${escapeAttr(t.duration || '')}" placeholder="MM:SS" data-track-id="${t.id}" onchange="saveTrackField(${parentId},${t.id},'duration',this.value,'${editType}')">
                    <div class="edit-track-audio">
                        ${t.file_hash
                            ? `<span class="edit-track-file${hasAudio}">${escapeHtml(t.file_name || t.file_hash)}</span>
                               <button class="btn btn-xs btn-ghost" onclick="playAudio('/audio/${t.file_hash}','${escapeAttr(t.title)}')" aria-label="再生">
                                   <span class="material-icons" aria-hidden="true">play_arrow</span>
                               </button>
                               <button class="btn btn-xs btn-outline-danger" onclick="deleteTrackAudio('${editType}',${parentId},${t.id})">消</button>`
                            : `<label class="btn btn-xs btn-outline-success" style="cursor:pointer">
                                   音声
                                   <input type="file" accept="audio/*" hidden onchange="uploadTrackAudio('${editType}',${parentId},${t.id},this)">
                               </label>`}
                    </div>
                    <div class="edit-track-reorder">
                        <button class="btn btn-xs btn-ghost" ${isFirst ? 'disabled' : ''} onclick="moveTrack(${parentId},${t.id},${discId},'up','${editType}')" title="上へ">&#9650;</button>
                        <button class="btn btn-xs btn-ghost" ${isLast ? 'disabled' : ''} onclick="moveTrack(${parentId},${t.id},${discId},'down','${editType}')" title="下へ">&#9660;</button>
                        <button class="btn btn-xs btn-outline-danger" onclick="removeTrack(${parentId},${t.id},'${editType}')" title="削除">&#10005;</button>
                    </div>
                </div>`;
        }).join("");
    }

    if (discKeys.length === 1) {
        const dVal = discKeys[0];
        html += `<div style="margin-top:0.4rem">
            <button class="btn btn-xs btn-outline-success" onclick="addTrackToDisc(${parentId},${dVal},'${editType}')">+ トラック追加</button>
        </div>`;
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

async function saveTrackField(parentId, trackId, field, value, editType) {
    const body = {};
    if (field === "title") {
        body.title = value;
    } else if (field === "duration") {
        body.duration = value;
    }
    const url = editType === "cd"
        ? `/api/cds/${parentId}/tracks/${trackId}`
        : `/api/books/${parentId}/tracks/${trackId}`;
    try {
        await fetch(url, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
        });
    } catch {}
}

async function addTrackToDisc(parentId, discNumber, editType) {
    if (editType !== "cd") return;
    const title = prompt("トラック名を入力:");
    if (!title || !title.trim()) return;

    const existing = [...document.querySelectorAll("#edit-tracks-list .edit-track-row")];
    const nextNum = existing.length + 1;

    try {
        const res = await fetch(`/api/cds/${parentId}/tracks`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
                title: title.trim(),
                disc_number: discNumber,
                track_number: nextNum,
            }),
        });
        if (res.ok) {
            loadAndRenderCdTracks(parentId);
        }
    } catch {}
}

async function removeTrack(parentId, trackId, editType) {
    if (editType !== "cd") return;
    const ok = await showConfirm({ message: "このトラックを削除しますか？", okLabel: "削除" });
    if (!ok) return;
    try {
        const res = await fetch(`/api/cds/${parentId}/tracks/${trackId}`, { method: "DELETE" });
        if (res.ok) {
            loadAndRenderCdTracks(parentId);
        }
    } catch {}
}

async function moveTrack(parentId, trackId, discNumber, direction, editType) {
    if (editType !== "cd") return;

    const rows = [...document.querySelectorAll("#edit-tracks-list .edit-track-row")];
    const idx = rows.findIndex((r) => parseInt(r.dataset.trackId, 10) === trackId);
    if (idx < 0) return;

    let swapIdx;
    if (direction === "up") {
        swapIdx = idx - 1;
    } else {
        swapIdx = idx + 1;
    }
    if (swapIdx < 0 || swapIdx >= rows.length) return;

    const thisRow = rows[idx];
    const swapRow = rows[swapIdx];

    const thisTitle = thisRow.querySelector(".edit-track-title-input").value;
    const thisDur = thisRow.querySelector(".edit-track-dur-input").value;
    const swapTitle = swapRow.querySelector(".edit-track-title-input").value;
    const swapDur = swapRow.querySelector(".edit-track-dur-input").value;
    const swapId = parseInt(swapRow.dataset.trackId, 10);

    const thisNewNum = swapIdx + 1;
    const swapNewNum = idx + 1;

    try {
        await Promise.all([
            fetch(`/api/cds/${parentId}/tracks/${trackId}`, {
                method: "PUT",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ title: thisTitle, duration: thisDur || null, disc_number: discNumber, track_number: thisNewNum }),
            }),
            fetch(`/api/cds/${parentId}/tracks/${swapId}`, {
                method: "PUT",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ title: swapTitle, duration: swapDur || null, disc_number: discNumber, track_number: swapNewNum }),
            }),
        ]);
    } catch {}
    loadAndRenderCdTracks(parentId);
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
