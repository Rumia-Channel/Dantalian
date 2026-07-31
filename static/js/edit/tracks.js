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
                <span>Disc ${d} <span class="edit-disc-count">(${discTracks.length} トラック)</span></span>
                <button type="button" class="btn btn-xs btn-outline-success" onclick="addTrackToDisc(${parentId},${d},'${editType}')">+ トラック追加</button>
            </div>`;
        }
        html += discTracks.map((t, idx) => {
            const isFirst = idx === 0;
            const isLast = idx === discTracks.length - 1;
            const discId = parseInt(d, 10);
            const hasAudio = t.file_hash ? ' has-audio' : '';
            const numLabel = discKeys.length > 1
                ? `${d}-${String(t.track_number).padStart(2, "0")}`
                : String(t.track_number).padStart(2, "0");
            return `
                <div class="edit-track-row" data-track-id="${t.id}" data-disc-number="${discId}">
                    <div class="edit-track-head">
                        <span class="edit-track-num" title="Disc ${d} / Track ${t.track_number}">${numLabel}</span>
                        <input type="text" class="edit-track-title-input" value="${escapeAttr(t.title)}" data-track-id="${t.id}" onchange="saveTrackField(${parentId},${t.id},'title',this.value,'${editType}')">
                        <input type="text" class="edit-track-dur-input" value="${escapeAttr(t.duration || '')}" placeholder="MM:SS" data-track-id="${t.id}" onchange="saveTrackField(${parentId},${t.id},'duration',this.value,'${editType}')">
                    </div>
                    <div class="edit-track-sub">
                        <div class="edit-track-audio">
                            ${t.file_hash
                                ? `<span class="edit-track-file${hasAudio}" title="${escapeAttr(t.file_name || t.file_hash)}">${escapeHtml(t.file_name || t.file_hash)}</span>
                                   <button type="button" class="btn btn-xs btn-ghost" onclick="playAudio('/audio/${t.file_hash}','${escapeJs(t.title)}')" aria-label="再生">
                                       <span class="material-icons" aria-hidden="true">play_arrow</span>
                                   </button>
                                   <button type="button" class="btn btn-xs btn-outline-danger" onclick="deleteTrackAudio('${editType}',${parentId},${t.id})" title="音声を削除">消</button>
                                   <label class="btn btn-xs btn-outline-success" style="cursor:pointer" title="音声を差し替え">
                                       差替
                                       <input type="file" accept="audio/mp3,audio/wav,audio/flac,audio/ogg,audio/m4a,audio/aac,audio/opus,audio/webm" hidden onchange="uploadTrackAudio('${editType}',${parentId},${t.id},this)">
                                   </label>
                                   <button type="button" class="btn btn-xs btn-ghost" onclick="showTrackMetadata('${editType}',${parentId},${t.id})" title="メタデータ表示">
                                       <span class="material-icons" aria-hidden="true">info</span>
                                   </button>`
                                : `<label class="btn btn-sm btn-outline-success" style="cursor:pointer" title="音声ファイルを登録（mp3/wav/flac/ogg/m4a/aac/opus/webm、設定画面の上限まで）">
                                       <span class="material-icons" aria-hidden="true">upload</span>
                                       音声
                                       <input type="file" accept="audio/mp3,audio/wav,audio/wma,audio/flac,audio/ogg,audio/m4a,audio/aac,audio/opus,audio/webm" hidden onchange="uploadTrackAudio('${editType}',${parentId},${t.id},this)">
                                   </label>`}
                        </div>
                        <div class="edit-track-reorder">
                            <button type="button" class="btn btn-xs btn-ghost" ${isFirst ? 'disabled' : ''} onclick="moveTrack(${parentId},${t.id},${discId},'up','${editType}')" title="上へ">&#9650;</button>
                            <button type="button" class="btn btn-xs btn-ghost" ${isLast ? 'disabled' : ''} onclick="moveTrack(${parentId},${t.id},${discId},'down','${editType}')" title="下へ">&#9660;</button>
                            <button type="button" class="btn btn-xs btn-outline-danger" onclick="removeTrack(${parentId},${t.id},'${editType}')" title="削除">&#10005;</button>
                        </div>
                    </div>
                </div>`;
        }).join("");
    }

    if (discKeys.length === 1) {
        const dVal = discKeys[0];
        const discCount = discGroups[dVal].length;
        html += `<div style="margin-top:0.4rem;display:flex;justify-content:space-between;align-items:center">
            <span class="edit-disc-count">Disc ${dVal} / ${discCount} トラック</span>
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
        const res = await fetch(url, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
        });
        if (!res.ok) {
            const error = await res.json().catch(() => ({}));
            alert(`トラック情報の保存に失敗しました (HTTP ${res.status}): ${error.error || ""}`);
        }
    } catch (err) {
        console.error("saveTrackField failed:", err);
        alert("トラック情報の保存中に通信エラーが発生しました");
    }
}

async function addTrackToDisc(parentId, discNumber, editType) {
    if (editType !== "cd") return;
    const title = prompt("トラック名を入力:");
    if (!title || !title.trim()) return;

    const inDisc = [...document.querySelectorAll(
        `#edit-tracks-list .edit-track-row[data-disc-number="${discNumber}"]`
    )];
    const nextNum = inDisc.length + 1;

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
    const sameDiscRows = rows.filter((row) => Number(row.dataset.discNumber) === Number(discNumber));
    const idx = sameDiscRows.findIndex((r) => parseInt(r.dataset.trackId, 10) === trackId);
    if (idx < 0) return;

    let swapIdx;
    if (direction === "up") {
        swapIdx = idx - 1;
    } else {
        swapIdx = idx + 1;
    }
    if (swapIdx < 0 || swapIdx >= sameDiscRows.length) return;

    const swapRow = sameDiscRows[swapIdx];
    const swapId = parseInt(swapRow.dataset.trackId, 10);

    try {
        const res = await fetch(`/api/cds/${parentId}/tracks/${trackId}`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ swap_track_id: swapId }),
        });
        if (!res.ok) {
            const errBody = await res.json().catch(() => ({}));
            console.error("moveTrack failed:", res.status, errBody);
            alert(`トラック順の変更に失敗しました (HTTP ${res.status}): ${errBody.error || ""}`);
        }
    } catch (err) {
        console.error("moveTrack error:", err);
        alert("トラック順の変更中に通信エラーが発生しました");
    }
    loadAndRenderCdTracks(parentId);
}

const ALLOWED_AUDIO_EXTS = ["mp3", "wav", "flac", "ogg", "m4a", "aac", "opus", "webm"];
const DEFAULT_AUDIO_MAX_MB = 100;
const MAX_UPLOAD_SETTING_MB = 4096;

async function getConfiguredAudioMaxMb() {
    try {
        const res = await fetch("/api/settings", { cache: "no-store" });
        if (res.ok) {
            const settings = await res.json();
            const configured = Number(settings["upload.audio_max_mb"]);
            if (Number.isSafeInteger(configured) && configured > 0) {
                return Math.min(configured, MAX_UPLOAD_SETTING_MB);
            }
        }
    } catch {}
    return DEFAULT_AUDIO_MAX_MB;
}

async function uploadTrackAudio(editType, parentId, trackId, input) {
    const file = input.files[0];
    input.value = "";
    if (!file) return;

    const ext = (file.name.split(".").pop() || "").toLowerCase();
    if (!ALLOWED_AUDIO_EXTS.includes(ext)) {
        alert(`対応していない拡張子です: .${ext}\n許可: ${ALLOWED_AUDIO_EXTS.map(e => "." + e).join(", ")}`);
        return;
    }
    const maxAudioMb = await getConfiguredAudioMaxMb();
    const maxAudioBytes = maxAudioMb * 1024 * 1024;
    if (file.size > maxAudioBytes) {
        alert(`ファイルが大きすぎます: ${(file.size / 1024 / 1024).toFixed(1)} MB\n上限: ${maxAudioMb} MB`);
        return;
    }

    const url = editType === "cd"
        ? `/api/cds/${parentId}/tracks/${trackId}/audio`
        : `/api/books/${parentId}/tracks/${trackId}/audio`;
    try {
        const res = await uploadFileWithChunks(url, "audio", file);
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

async function showTrackMetadata(editType, parentId, trackId) {
    const url = editType === "cd"
        ? `/api/cds/${parentId}/tracks/${trackId}/metadata`
        : `/api/books/${parentId}/tracks/${trackId}/metadata`;
    let meta = {};
    try {
        const res = await fetch(url);
        if (res.ok) {
            const data = await res.json();
            if (data && typeof data === "object") meta = data;
        }
    } catch (err) {
        console.error("showTrackMetadata fetch failed:", err);
    }

    const isCd = editType === "cd";

    // CD の track_metadata 由来タグ合意(参考)。cd 側が空の項目は仮入力にも使う。
    let tags = {};
    if (isCd) {
        try {
            const tr = await fetch(`/api/cds/${parentId}/album-tags`);
            if (tr.ok) {
                const tj = await tr.json();
                if (tj && typeof tj === "object") tags = tj;
            }
        } catch (err) {
            console.error("loadCdAlbumTags (modal) failed:", err);
        }
        for (const k of ["composer", "genre", "year"]) {
            if ((meta[k] == null || String(meta[k]) === "") && tags[k] != null && String(tags[k]) !== "") {
                meta[k] = tags[k];
            }
        }
    }

    const cdLevelFields = [
        { key: "composer", label: "作曲" },
        { key: "genre", label: "ジャンル" },
        { key: "year", label: "年", type: "number", min: 1000, max: 9999 },
        { key: "isrc", label: "ISRC" },
    ];
    const trackLevelFields = [
        { key: "title", label: "タイトル" },
        { key: "track_number", label: "トラック番号", type: "number", min: 1 },
        { key: "track_total", label: "トラック総数", type: "number", min: 1 },
        { key: "disc_number", label: "ディスク番号", type: "number", min: 1 },
        { key: "disc_total", label: "ディスク総数", type: "number", min: 1 },
        { key: "comment", label: "コメント" },
        { key: "lyrics", label: "歌詞", type: "textarea" },
        { key: "encoder", label: "エンコーダ" },
    ];

    function rowFor(f, scope) {
        const v = meta[f.key];
        const val = v == null ? "" : String(v);
        const type = f.type || "text";
        const min = f.min != null ? `min="${f.min}"` : "";
        const max = f.max != null ? `max="${f.max}"` : "";
        if (type === "textarea") {
            return `<tr><th><label for="meta-field-${f.key}">${escapeHtml(f.label)}</label></th>
                <td><textarea id="meta-field-${f.key}" data-meta-key="${f.key}" data-meta-scope="${scope}" rows="4">${escapeHtml(val)}</textarea></td></tr>`;
        }
        return `<tr><th><label for="meta-field-${f.key}">${escapeHtml(f.label)}</label></th>
            <td><input type="${type}" id="meta-field-${f.key}" data-meta-key="${f.key}" data-meta-scope="${scope}" value="${escapeAttr(val)}" ${min} ${max}></td></tr>`;
    }

    const cdLevelRows = cdLevelFields.map((f) => rowFor(f, "cd")).join("");
    const trackLevelRows = trackLevelFields.map((f) => rowFor(f, "track")).join("");

    const tagRefRows = [
        ["アルバム名", tags.album],
        ["アルバムアーティスト", tags.album_artist],
        ["出版社", tags.publisher],
        ["レーベル", tags.label],
    ].filter(([, v]) => v != null && String(v) !== "");
    const tagRefHtml = tagRefRows.length > 0
        ? `<div class="edit-tag-ref" style="margin:0.5rem 0">
               <div class="edit-tag-ref-title">音声タグ由来 (参考・このモーダルでは編集不可)</div>
               ${tagRefRows.map(([l, v]) => `<div class="edit-tag-ref-row"><span class="edit-tag-ref-label">${escapeHtml(l)}</span><span class="edit-tag-ref-value">${escapeHtml(v)}</span></div>`).join("")}
           </div>`
        : "";

    const albumArtists = Array.isArray(meta.album_artists) ? meta.album_artists : [];
    const albumArtistListHtml = albumArtists.length > 0
        ? albumArtists.map((a) => `<span class="meta-pill">${escapeHtml(a.name)}${a.transcription ? ` <span class="meta-pill-yomi">(${escapeHtml(a.transcription)})</span>` : ""}</span>`).join("")
        : `<span style="color:var(--color-text-dim);font-size:0.85rem">CD 基本情報の「アーティスト」セクションで追加してください</span>`;

    const overlay = document.createElement("div");
    overlay.className = "confirm-overlay";
    overlay.innerHTML = `
        <div class="confirm-box" style="max-width:620px;text-align:left;max-height:80vh;overflow-y:auto">
            <div class="confirm-message" style="font-weight:600;margin-bottom:0.4rem">トラックメタデータ編集</div>
            ${isCd ? `
            <div style="font-size:0.75rem;color:var(--color-text-dim);margin-bottom:0.4rem">
                「<strong>アルバム情報</strong>」を編集すると CD 全体に反映されます。「<strong>トラック情報</strong>」はこのトラックのみ。
            </div>
            <div style="font-size:0.85rem;font-weight:600;margin-top:0.5rem">アルバムアーティスト (CD 基本情報と共有・編集は CD 編集画面で)</div>
            <div id="meta-album-artists" style="margin:0.3rem 0 0.5rem 0;line-height:1.8">${albumArtistListHtml}</div>
            ${tagRefHtml}
            <div style="font-size:0.85rem;font-weight:600;margin-top:0.5rem">アルバム情報 (audio 固有・全トラックで共有)</div>
            <table class="edit-meta-table">${cdLevelRows}</table>
            <div style="font-size:0.85rem;font-weight:600;margin-top:0.8rem">トラック情報 (このトラックのみ)</div>
            <div style="font-size:0.75rem;color:var(--color-text-dim);margin-bottom:0.2rem">複数可・登録済みアーティストから選択 (未登録も新規追加可)</div>
            <div id="meta-track-artists-list" class="meta-author-list"></div>
            <div class="meta-author-add">
                <div id="meta-track-artists-select"></div>
                <button type="button" class="btn btn-xs btn-outline-success" id="meta-track-artists-add">追加</button>
            </div>
            <table class="edit-meta-table" style="margin-top:0.5rem">${trackLevelRows}</table>
            ` : `
            <div style="font-size:0.85rem;font-weight:600;margin-top:0.5rem">トラック情報 (このトラックのみ)</div>
            <div id="meta-track-artists-list" class="meta-author-list"></div>
            <div class="meta-author-add">
                <div id="meta-track-artists-select"></div>
                <button type="button" class="btn btn-xs btn-outline-success" id="meta-track-artists-add">追加</button>
            </div>
            <table class="edit-meta-table" style="margin-top:0.5rem">${trackLevelRows}</table>
            `}
            <div class="confirm-actions" style="margin-top:0.8rem;justify-content:flex-end;gap:0.4rem">
                <button type="button" class="btn btn-sm btn-ghost" id="meta-modal-close">キャンセル</button>
                <button type="button" class="btn btn-sm btn-outline-danger" id="meta-modal-clear">クリア</button>
                <button type="button" class="btn btn-sm btn-outline-success" id="meta-modal-save">保存</button>
            </div>
        </div>
    `;
    document.body.appendChild(overlay);

    const trackAuthorIds = (Array.isArray(meta.artists) ? meta.artists : []).map((a) => a.id);
    const trackAuthorNames = new Map((Array.isArray(meta.artists) ? meta.artists : []).map((a) => [a.id, a.name]));
    const trackArtistsList = document.getElementById("meta-track-artists-list");
    function renderTrackAuthorList() {
        if (!trackArtistsList) return;
        if (trackAuthorIds.length === 0) {
            trackArtistsList.innerHTML = `<p class="series-empty" style="margin:0.2rem 0">アーティスト未登録</p>`;
            return;
        }
        trackArtistsList.innerHTML = trackAuthorIds.map((id, idx) => `
            <div class="meta-author-item" data-author-id="${id}">
                <span class="meta-author-name">${escapeHtml(trackAuthorNames.get(id) || `ID:${id}`)}</span>
                <button type="button" class="btn btn-xs btn-outline-danger" data-rm="${id}">削除</button>
                <input type="number" class="meta-author-order" value="${idx}" min="0" step="1" data-ord="${id}" title="並び順">
            </div>
        `).join("");
        trackArtistsList.querySelectorAll("[data-rm]").forEach((b) => {
            b.addEventListener("click", () => {
                const id = parseInt(b.getAttribute("data-rm"), 10);
                trackAuthorIds.splice(trackAuthorIds.indexOf(id), 1);
                trackAuthorNames.delete(id);
                renderTrackAuthorList();
            });
        });
    }
    renderTrackAuthorList();

    let trackAuthorSelect = null;
    const selectContainer = document.getElementById("meta-track-artists-select");
    if (selectContainer && typeof createSearchableSelect === "function") {
        trackAuthorSelect = createSearchableSelect(selectContainer, {
            options: (typeof allAuthors !== "undefined" ? allAuthors : []).map((a) => ({ value: a.id, label: a.name })),
            value: null,
            placeholder: "アーティストを追加...",
            clearable: false,
        });
    }
    const addBtn = document.getElementById("meta-track-artists-add");
    if (addBtn) {
        addBtn.addEventListener("click", () => {
            if (!trackAuthorSelect) return;
            const id = trackAuthorSelect.getValue();
            if (!id) return;
            if (trackAuthorIds.includes(id)) return;
            trackAuthorIds.push(id);
            const a = (typeof allAuthors !== "undefined" ? allAuthors : []).find((x) => x.id === id);
            if (a) trackAuthorNames.set(id, a.name);
            trackAuthorSelect.setValue(null);
            renderTrackAuthorList();
        });
    }

    await new Promise((resolve) => {
        const close = () => {
            overlay.classList.add("hidden");
            setTimeout(() => overlay.remove(), 200);
            resolve();
        };
        overlay.querySelector("#meta-modal-close").addEventListener("click", close);
        overlay.querySelector("#meta-modal-clear").addEventListener("click", () => {
            overlay.querySelectorAll("input[data-meta-key], textarea[data-meta-key]").forEach((i) => { i.value = ""; });
            trackAuthorIds.length = 0;
            trackAuthorNames.clear();
            renderTrackAuthorList();
        });
        overlay.querySelector("#meta-modal-save").addEventListener("click", async () => {
            const cdBody = {};
            const trackBody = { artists: trackAuthorIds.slice() };
            overlay.querySelectorAll("input[data-meta-key], textarea[data-meta-key]").forEach((i) => {
                const k = i.dataset.metaKey;
                const scope = i.dataset.metaScope;
                const v = i.value.trim();
                if (v === "") return;
                if (i.type === "number") {
                    const n = parseInt(v, 10);
                    if (!Number.isFinite(n)) return;
                    (scope === "cd" ? cdBody : trackBody)[k] = n;
                } else {
                    (scope === "cd" ? cdBody : trackBody)[k] = v;
                }
            });

            let allOk = true;
            if (isCd && Object.keys(cdBody).length > 0) {
                try {
                    const r = await fetch(`/api/cds/${parentId}/metadata`, {
                        method: "PUT",
                        headers: { "Content-Type": "application/json" },
                        body: JSON.stringify(cdBody),
                    });
                    if (!r.ok) {
                        allOk = false;
                        const err = await r.json().catch(() => ({}));
                        alert(`アルバム情報の保存に失敗しました (HTTP ${r.status}): ${err.error || ""}`);
                    }
                } catch (err) {
                    allOk = false;
                    console.error("cd meta save error:", err);
                    alert("アルバム情報の通信エラーが発生しました");
                }
            }
            if (allOk) {
                const saveUrl = editType === "cd"
                    ? `/api/cds/${parentId}/tracks/${trackId}/metadata`
                    : `/api/books/${parentId}/tracks/${trackId}/metadata`;
                try {
                    const r = await fetch(saveUrl, {
                        method: "PUT",
                        headers: { "Content-Type": "application/json" },
                        body: JSON.stringify(trackBody),
                    });
                    if (!r.ok) {
                        allOk = false;
                        const err = await r.json().catch(() => ({}));
                        alert(`トラック情報の保存に失敗しました (HTTP ${r.status}): ${err.error || ""}`);
                    }
                } catch (err) {
                    allOk = false;
                    console.error("track meta save error:", err);
                    alert("トラック情報の通信エラーが発生しました");
                }
            }
            if (allOk) {
                if (Object.keys(cdBody).length > 0 || trackAuthorIds.length > 0 || Object.keys(trackBody).length > 1) {
                    alert("メタデータを保存しました");
                }
                close();
            }
        });
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

    const isCd = editType === "cd";
    const overlay = document.createElement("div");
    overlay.className = "confirm-overlay";
    overlay.innerHTML = `
        <div class="confirm-box" style="max-width:520px;text-align:left">
            <div class="confirm-message" style="font-weight:600;margin-bottom:0.6rem">${isCd ? "抽出したメタデータをCD／曲情報へ反映しました" : "抽出したメタデータ"}</div>
            <table class="edit-meta-table">${rows}</table>
            <div class="confirm-actions" style="margin-top:0.8rem;justify-content:flex-end">
                <button type="button" class="btn btn-sm btn-ghost" id="meta-modal-skip">閉じる</button>
                ${isCd || !meta.title ? "" : '<button type="button" class="btn btn-sm btn-outline-success" id="meta-modal-apply-title">タイトルを反映</button>'}
            </div>
        </div>
    `;
    document.body.appendChild(overlay);

    await new Promise((resolve) => {
        const close = () => {
            overlay.classList.add("hidden");
            setTimeout(() => overlay.remove(), 200);
            resolve();
        };
        overlay.querySelector("#meta-modal-skip").addEventListener("click", close);
        const applyBtn = overlay.querySelector("#meta-modal-apply-title");
        if (applyBtn && meta.title && !isCd) {
            applyBtn.addEventListener("click", async () => {
                await saveTrackField(parentId, trackId, "title", meta.title, editType);
                close();
                await reloadFn(parentId);
            });
        }
        overlay.addEventListener("click", (e) => {
            if (e.target === overlay) close();
        });
    });
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
