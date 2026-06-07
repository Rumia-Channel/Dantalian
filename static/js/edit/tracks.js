function renderTracksHtml(tracks, editType, parentId) {
    if (tracks.length === 0) {
        if (editType === "cd") {
            return `<p class='series-empty'>トラックなし</p>
                <div style="margin-top:0.4rem">
                    <button type="button" class="btn btn-xs btn-outline-success" onclick="addTrackToDisc(${parentId},1,'${editType}')">+ トラック追加</button>
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
                <button type="button" class="btn btn-xs btn-outline-success" onclick="addTrackToDisc(${parentId},${d},'${editType}')">+ トラック追加</button>
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
                            ? `<span class="edit-track-file${hasAudio}" title="${escapeAttr(t.file_name || t.file_hash)}">${escapeHtml(t.file_name || t.file_hash)}</span>
                               <button type="button" class="btn btn-xs btn-ghost" onclick="playAudio('/audio/${t.file_hash}','${escapeAttr(t.title)}')" aria-label="再生">
                                   <span class="material-icons" aria-hidden="true">play_arrow</span>
                               </button>
                               <button type="button" class="btn btn-xs btn-outline-danger" onclick="deleteTrackAudio('${editType}',${parentId},${t.id})" title="音声を削除">消</button>
                               <label class="btn btn-xs btn-outline-success" style="cursor:pointer" title="音声を差し替え">
                                   差替
                                   <input type="file" accept="audio/mp3,audio/wav,audio/flac,audio/ogg,audio/m4a,audio/aac,audio/opus,audio/webm" hidden onchange="uploadTrackAudio('${editType}',${parentId},${t.id},this)">
                               </label>`
                            : `<label class="btn btn-sm btn-outline-success" style="cursor:pointer" title="音声ファイルを登録（mp3/wav/flac/ogg/m4a/aac/opus/webm、最大 100 MB）">
                                   <span class="material-icons" aria-hidden="true">upload</span>
                                   音声
                                   <input type="file" accept="audio/mp3,audio/wav,audio/flac,audio/ogg,audio/m4a,audio/aac,audio/opus,audio/webm" hidden onchange="uploadTrackAudio('${editType}',${parentId},${t.id},this)">
                               </label>`}
                    </div>
                    <div class="edit-track-reorder">
                        <button type="button" class="btn btn-xs btn-ghost" ${isFirst ? 'disabled' : ''} onclick="moveTrack(${parentId},${t.id},${discId},'up','${editType}')" title="上へ">&#9650;</button>
                        <button type="button" class="btn btn-xs btn-ghost" ${isLast ? 'disabled' : ''} onclick="moveTrack(${parentId},${t.id},${discId},'down','${editType}')" title="下へ">&#9660;</button>
                        <button type="button" class="btn btn-xs btn-outline-danger" onclick="removeTrack(${parentId},${t.id},'${editType}')" title="削除">&#10005;</button>
                    </div>
                </div>`;
        }).join("");
    }

    if (discKeys.length === 1) {
        const dVal = discKeys[0];
        html += `<div style="margin-top:0.4rem">
            <button type="button" class="btn btn-xs btn-outline-success" onclick="addTrackToDisc(${parentId},${dVal},'${editType}')">+ トラック追加</button>
        </div>`;
    }

    return html;
}

async function loadAndRenderTracks(bookId) {
    const list = document.getElementById("edit-tracks-list");
    if (!list) return;
    try {
        const res = await fetch(`/api/books/${bookId}/tracks`);
        if (!res.ok) { list.innerHTML = `<p class='series-empty'>トラック読み込み失敗 (HTTP ${res.status})</p>`; return; }
        const tracks = await res.json();
        list.innerHTML = renderTracksHtml(tracks, "book", bookId);
    } catch (err) {
        console.error("loadAndRenderTracks failed:", err);
        list.innerHTML = "<p class='series-empty'>トラック読み込みエラー</p>";
    }
}

async function loadAndRenderCdTracks(cdId) {
    const list = document.getElementById("edit-tracks-list");
    if (!list) return;
    try {
        const res = await fetch(`/api/cds/${cdId}/tracks`);
        if (!res.ok) {
            const errBody = await res.json().catch(() => ({}));
            console.error("loadAndRenderCdTracks failed:", res.status, errBody);
            list.innerHTML = `<p class='series-empty'>トラック読み込み失敗 (HTTP ${res.status})</p>`;
            return;
        }
        const tracks = await res.json();
        list.innerHTML = renderTracksHtml(tracks, "cd", cdId);
    } catch (err) {
        console.error("loadAndRenderCdTracks failed:", err);
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
        } else {
            const errBody = await res.json().catch(() => ({}));
            console.error("addTrack failed:", res.status, errBody);
            alert(`トラックの追加に失敗しました (HTTP ${res.status}): ${errBody.error || ""}`);
        }
    } catch (err) {
        console.error("addTrack error:", err);
        alert("トラックの追加中に通信エラーが発生しました");
    }
}

async function removeTrack(parentId, trackId, editType) {
    if (editType !== "cd") return;
    const ok = await showConfirm({ message: "このトラックを削除しますか？", okLabel: "削除" });
    if (!ok) return;
    try {
        const res = await fetch(`/api/cds/${parentId}/tracks/${trackId}`, { method: "DELETE" });
        if (res.ok || res.status === 204) {
            loadAndRenderCdTracks(parentId);
        } else {
            const errBody = await res.json().catch(() => ({}));
            console.error("removeTrack failed:", res.status, errBody);
            alert(`トラックの削除に失敗しました (HTTP ${res.status}): ${errBody.error || ""}`);
        }
    } catch (err) {
        console.error("removeTrack error:", err);
        alert("トラックの削除中に通信エラーが発生しました");
    }
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
        const results = await Promise.all([
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
        for (const r of results) {
            if (!r.ok) {
                const errBody = await r.json().catch(() => ({}));
                console.error("moveTrack failed:", r.status, errBody);
                alert(`トラック順の変更に失敗しました (HTTP ${r.status})`);
                break;
            }
        }
    } catch (err) {
        console.error("moveTrack error:", err);
        alert("トラック順の変更中に通信エラーが発生しました");
    }
    loadAndRenderCdTracks(parentId);
}

const ALLOWED_AUDIO_EXTS = ["mp3", "wav", "flac", "ogg", "m4a", "aac", "opus", "webm"];
const MAX_AUDIO_BYTES = 100 * 1024 * 1024;

async function uploadTrackAudio(editType, parentId, trackId, input) {
    const file = input.files[0];
    input.value = "";
    if (!file) return;

    const ext = (file.name.split(".").pop() || "").toLowerCase();
    if (!ALLOWED_AUDIO_EXTS.includes(ext)) {
        alert(`対応していない拡張子です: .${ext}\n許可: ${ALLOWED_AUDIO_EXTS.map(e => "." + e).join(", ")}`);
        return;
    }
    if (file.size > MAX_AUDIO_BYTES) {
        alert(`ファイルが大きすぎます: ${(file.size / 1024 / 1024).toFixed(1)} MB\n上限: ${MAX_AUDIO_BYTES / 1024 / 1024} MB`);
        return;
    }

    const fd = new FormData();
    fd.append("audio", file);
    const url = editType === "cd"
        ? `/api/cds/${parentId}/tracks/${trackId}/audio`
        : `/api/books/${parentId}/tracks/${trackId}/audio`;
    try {
        const res = await fetch(url, { method: "POST", body: fd });
        if (res.ok) {
            const body = await res.json().catch(() => ({}));
            const reloadFn = editType === "cd" ? loadAndRenderCdTracks : loadAndRenderTracks;
            await reloadFn(parentId);
            if (body.file_hash) {
                await offerToApplyAudioDuration(editType, parentId, trackId, body.file_hash, reloadFn);
            }
            if (body.metadata) {
                await showExtractedMetadataModal(editType, parentId, trackId, body.metadata, reloadFn);
            }
        } else {
            const errBody = await res.json().catch(() => ({}));
            console.error("uploadTrackAudio failed:", res.status, errBody);
            alert(`音声のアップロードに失敗しました (HTTP ${res.status})`);
        }
    } catch (err) {
        console.error("uploadTrackAudio error:", err);
        alert("音声のアップロード中に通信エラーが発生しました");
    }
}

async function showExtractedMetadataModal(editType, parentId, trackId, meta, reloadFn) {
    if (!meta || typeof meta !== "object") return;
    const fields = [
        ["タイトル", meta.title],
        ["アーティスト", meta.artist],
        ["アルバム", meta.album],
        ["アルバムアーティスト", meta.album_artist],
        ["作曲者", meta.composer],
        ["ジャンル", meta.genre],
        ["年", meta.year],
        ["トラック番号", meta.track_number],
        ["ディスク番号", meta.disc_number],
        ["出版社", meta.publisher],
        ["レーベル", meta.label],
    ];
    const rows = fields
        .filter(([, v]) => v != null && v !== "")
        .map(([k, v]) => `<tr><th>${escapeHtml(k)}</th><td>${escapeHtml(String(v))}</td></tr>`)
        .join("");
    if (!rows) return;

    const html = `
        <div class="confirm-box" style="max-width:520px;text-align:left">
            <div class="confirm-message" style="font-weight:600;margin-bottom:0.6rem">抽出したメタデータ</div>
            <table class="edit-meta-table">${rows}</table>
            <div class="confirm-actions" style="margin-top:0.8rem;justify-content:flex-end">
                <button type="button" class="btn btn-sm btn-ghost" id="meta-modal-skip">閉じる</button>
                <button type="button" class="btn btn-sm btn-outline-success" id="meta-modal-apply-title">${meta.title ? "タイトルを反映" : "閉じる"}</button>
            </div>
        </div>
    `;
    const overlay = document.createElement("div");
    overlay.className = "confirm-overlay";
    overlay.innerHTML = html;
    document.body.appendChild(overlay);

    await new Promise((resolve) => {
        const close = () => {
            overlay.classList.add("hidden");
            setTimeout(() => overlay.remove(), 200);
            resolve();
        };
        overlay.querySelector("#meta-modal-skip").addEventListener("click", close);
        const applyBtn = overlay.querySelector("#meta-modal-apply-title");
        if (meta.title) {
            applyBtn.addEventListener("click", async () => {
                await saveTrackField(parentId, trackId, "title", meta.title, editType);
                close();
                await reloadFn(parentId);
            });
        } else {
            applyBtn.style.display = "none";
        }
        overlay.addEventListener("click", (e) => {
            if (e.target === overlay) close();
        });
    });
}

function secondsToMmSs(secs) {
    if (!isFinite(secs) || secs <= 0) return null;
    const total = Math.round(secs);
    const m = Math.floor(total / 60);
    const s = total % 60;
    return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

function probeAudioDuration(url) {
    return new Promise((resolve) => {
        const a = new Audio();
        a.preload = "metadata";
        a.src = url;
        const cleanup = () => {
            a.onloadedmetadata = null;
            a.onerror = null;
        };
        a.onloadedmetadata = () => {
            const d = a.duration;
            cleanup();
            resolve(d);
        };
        a.onerror = () => {
            cleanup();
            resolve(NaN);
        };
    });
}

async function offerToApplyAudioDuration(editType, parentId, trackId, fileHash, reloadFn) {
    const secs = await probeAudioDuration(`/audio/${fileHash}?_=${Date.now()}`);
    const formatted = secondsToMmSs(secs);
    if (!formatted) return;
    const ok = await showConfirm({
        message: `音声ファイルの長さ ${formatted} をトラックの長さに設定しますか？`,
        okLabel: "設定する",
        cancelLabel: "設定しない",
        okClass: "btn btn-sm btn-outline-success",
    });
    if (!ok) return;
    await saveTrackField(parentId, trackId, "duration", formatted, editType);
    await reloadFn(parentId);
}

async function deleteTrackAudio(editType, parentId, trackId) {
    if (!await showConfirm({ message: "音声ファイルを削除しますか？", okLabel: "削除" })) return;
    const url = editType === "cd"
        ? `/api/cds/${parentId}/tracks/${trackId}/audio`
        : `/api/books/${parentId}/tracks/${trackId}/audio`;
    try {
        const res = await fetch(url, { method: "DELETE" });
        if (res.ok) {
            if (editType === "cd") loadAndRenderCdTracks(parentId);
            else loadAndRenderTracks(parentId);
        } else {
            const errBody = await res.json().catch(() => ({}));
            console.error("deleteTrackAudio failed:", res.status, errBody);
            alert(`音声の削除に失敗しました (HTTP ${res.status})`);
        }
    } catch (err) {
        console.error("deleteTrackAudio error:", err);
        alert("音声の削除中に通信エラーが発生しました");
    }
}
