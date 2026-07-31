const editContent = document.getElementById("edit-content");

const params = new URLSearchParams(window.location.search);
const bookId = params.get("book");
const cdId = params.get("cd");

let allAuthors = [];
var allBorrowers = [];
let editAuthorSelect = null;
let editCdSeriesSelect = null;
let editCdAuthorSelect = null;

(async () => {
    await loadBooks();
    await loadCds();
    await loadSeries();
    await loadGrandSeries();
    await loadStorageLocations();
    await loadLabels();

    const [aRes, bRes] = await Promise.all([
        fetch("/api/authors"),
        fetch("/api/borrowers"),
    ]);
    if (aRes.ok) allAuthors = await aRes.json();
    if (bRes.ok) allBorrowers = await bRes.json();

    if (cdId) {
        renderCdEdit(parseInt(cdId, 10));
    } else if (bookId) {
        renderBookEdit(parseInt(bookId, 10));
    } else {
        renderBookSelect();
    }
})();

function renderBookSelect() {
    editContent.innerHTML = `
        <h2>書籍編集</h2>
        <p class="series-empty">書籍詳細モーダルの「編集」ボタンから遷移してください</p>
    `;
}

function renderBookEdit(id) {
    const book = allBooks.find((b) => b.id === id);
    if (!book) {
        editContent.innerHTML = '<p class="empty-state">書籍が見つかりません</p>';
        return;
    }

    const authorList = book.authors || [];
    const currentGrandSeries = findBookGrandSeries(book.id);
    const linkedAuthorIds = new Set(authorList.map((a) => a.id));
    const availableAuthors = allAuthors.filter((a) => !linkedAuthorIds.has(a.id));

    editContent.innerHTML = `
        <h2>書籍情報編集</h2>
        <div class="edit-header">
            <div class="edit-cover-wrap">
                <div class="edit-cover" id="edit-cover-display">
                    ${book.cover_url
                        ? `<img class="book-cover" src="/images/${book.cover_url}" alt="">`
                        : '<div class="book-cover-placeholder">No Image</div>'}
                </div>
                <div class="edit-cover-actions">
                    <label class="btn btn-xs btn-outline-success edit-cover-upload-label">
                        変更
                        <input type="file" id="edit-cover-input" accept="image/*" hidden>
                    </label>
                    ${book.cover_url ? `<button type="button" class="btn btn-xs btn-outline-danger" onclick="deleteCover(${book.id})">削除</button>` : ""}
                </div>
            </div>
            <div class="edit-header-info">
                ${book.isbn ? `<div class="edit-isbn">${escapeHtml(book.isbn)}</div>` : ''}
                ${book.isdn ? `<div class="edit-isbn">ISDN: ${escapeHtml(book.isdn)}</div>` : ''}
                ${book.jan ? `<div class="edit-isbn">JAN: ${escapeHtml(book.jan)}</div>` : ''}
                <div class="edit-isbn" style="font-size:0.75rem;color:var(--color-text-dim);">種別: ${escapeHtml(book.media_type || 'book')}</div>
            </div>
        </div>
        <form class="edit-form" id="edit-form">
            <input type="hidden" name="series_id" value="${book.series_id != null ? book.series_id : ''}">
            <input type="hidden" name="grand_series_id" value="${currentGrandSeries ? currentGrandSeries.id : ''}">
            <input type="hidden" name="storage_location_id" value="${book.storage_location_id != null ? book.storage_location_id : ''}">
            <input type="hidden" name="label_id" value="${book.label_id != null ? book.label_id : ''}">
            <div class="edit-field">
                <label>タイトル <span class="edit-required">*</span></label>
                <input type="text" name="title" value="${escapeAttr(book.title)}" required>
            </div>
            <div class="edit-field">
                <label>タイトル(よみ)</label>
                <input type="text" name="title_transcription" value="${escapeAttr(book.title_transcription || '')}">
            </div>
            <div class="edit-row">
                <div class="edit-field">
                    <label>出版社 / サークル名</label>
                    <input type="text" name="publisher" value="${escapeAttr(book.publisher || '')}">
                </div>
                <div class="edit-field">
                    <label>出版日 / 発行日</label>
                    <input type="text" name="publish_date" value="${escapeAttr(book.publish_date || '')}">
                </div>
            </div>
            <div class="edit-row">
                <div class="edit-field">
                    <label>価格</label>
                    <input type="text" name="price" value="${escapeAttr(book.price || '')}">
                </div>
                <div class="edit-field">
                    <label>ページ数 / 体裁</label>
                    <input type="text" name="extent" value="${escapeAttr(book.extent || '')}">
                </div>
            </div>
            <div class="edit-row">
                <div class="edit-field">
                    <label>巻</label>
                    <input type="text" name="volume" value="${escapeAttr(book.volume || '')}">
                </div>
                <div class="edit-field">
                    <label>巻(よみ)</label>
                    <input type="text" name="volume_transcription" value="${escapeAttr(book.volume_transcription || '')}">
                </div>
            </div>
            <div class="edit-field">
                <label>別タイトル</label>
                <input type="text" name="alternative" value="${escapeAttr(book.alternative || '')}">
            </div>
            <div class="edit-field">
                <label>別タイトル(よみ)</label>
                <input type="text" name="alternative_transcription" value="${escapeAttr(book.alternative_transcription || '')}">
            </div>
            <div class="edit-field">
                <label>説明</label>
                <textarea name="description" rows="6">${escapeHtml(book.description || '')}</textarea>
            </div>
            <div class="edit-section">
                <h3 class="edit-section-title">ISBN / NDL 固有</h3>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>ISBN</label>
                        <input type="text" name="isbn" value="${escapeAttr(book.isbn || '')}">
                    </div>
                    <div class="edit-field">
                        <label>JPNO</label>
                        <input type="text" name="jpno" value="${escapeAttr(book.jpno || '')}">
                    </div>
                </div>
                <div class="edit-field">
                    <label>NDL URL</label>
                    <input type="text" name="ndl_url" value="${escapeAttr(book.ndl_url || '')}">
                </div>
            </div>
            <div class="edit-section">
                <h3 class="edit-section-title">ISDN 固有</h3>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>ISDN</label>
                        <input type="text" name="isdn" value="${escapeAttr(book.isdn || '')}">
                    </div>
                    <div class="edit-field">
                        <label>Cコード</label>
                        <input type="text" name="isdn_c_code" value="${escapeAttr(book.isdn_c_code || '')}">
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>区分</label>
                        <input type="text" name="isdn_class" value="${escapeAttr(book.isdn_class || '')}">
                    </div>
                    <div class="edit-field">
                        <label>形態</label>
                        <input type="text" name="isdn_type" value="${escapeAttr(book.isdn_type || '')}">
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>レーティング(性別)</label>
                        <input type="text" name="isdn_rating_gender" value="${escapeAttr(book.isdn_rating_gender || '')}">
                    </div>
                    <div class="edit-field">
                        <label>レーティング(年齢)</label>
                        <input type="text" name="isdn_rating_age" value="${escapeAttr(book.isdn_rating_age || '')}">
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>地域</label>
                        <input type="text" name="isdn_region" value="${escapeAttr(book.isdn_region || '')}">
                    </div>
                    <div class="edit-field">
                        <label>ジャンルコード</label>
                        <input type="text" name="isdn_genre_code" value="${escapeAttr(book.isdn_genre_code || '')}">
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>ジャンル名</label>
                        <input type="text" name="isdn_genre_name" value="${escapeAttr(book.isdn_genre_name || '')}">
                    </div>
                    <div class="edit-field">
                        <label>ジャンル補足</label>
                        <input type="text" name="isdn_genre_user" value="${escapeAttr(book.isdn_genre_user || '')}">
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>販売対象</label>
                        <input type="text" name="isdn_author" value="${escapeAttr(book.isdn_author || '')}">
                    </div>
                    <div class="edit-field">
                        <label>書籍形態(Cコード)</label>
                        <input type="text" name="isdn_shape" value="${escapeAttr(book.isdn_shape || '')}">
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>内容(Cコード)</label>
                        <input type="text" name="isdn_contents" value="${escapeAttr(book.isdn_contents || '')}">
                    </div>
                    <div class="edit-field">
                        <label>バーコード2段目</label>
                        <input type="text" name="isdn_barcode2" value="${escapeAttr(book.isdn_barcode2 || '')}">
                    </div>
                </div>
                <div class="edit-field">
                    <label>サンプル画像URL</label>
                    <input type="text" name="isdn_sample_image_url" value="${escapeAttr(book.isdn_sample_image_url || '')}">
                </div>
            </div>
            <div class="edit-section">
                <h3 class="edit-section-title">メディア</h3>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>種別</label>
                        <select name="media_type" class="form-input">
                            <option value="book" ${(!book.media_type || book.media_type === 'book') ? 'selected' : ''}>書籍</option>
                            <option value="cd" ${book.media_type === 'cd' ? 'selected' : ''}>CD</option>
                            <option value="audiobook" ${book.media_type === 'audiobook' ? 'selected' : ''}>オーディオブック</option>
                        </select>
                    </div>
                    <div class="edit-field">
                        <label>JAN</label>
                        <input type="text" name="jan" value="${escapeAttr(book.jan || '')}">
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>アーティスト</label>
                        <input type="text" name="artist" value="${escapeAttr(book.artist || '')}">
                    </div>
                    <div class="edit-field">
                        <label>レーベル</label>
                        <input type="text" name="label" value="${escapeAttr(book.label || '')}">
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>品番</label>
                        <input type="text" name="catalog_number" value="${escapeAttr(book.catalog_number || '')}">
                    </div>
                    <div class="edit-field">
                        <label>ディスク枚数</label>
                        <input type="number" name="disc_count" value="${book.disc_count != null ? book.disc_count : ''}" min="1" step="1">
                    </div>
                </div>
            </div>
            <div class="edit-section">
                <h3 class="edit-section-title">ステータス</h3>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>読書状況</label>
                        <select name="reading_status" class="form-input">
                            <option value="unread" ${(!book.reading_status || book.reading_status === 'unread') ? 'selected' : ''}>未読</option>
                            <option value="reading" ${book.reading_status === 'reading' ? 'selected' : ''}>読書中</option>
                            <option value="completed" ${book.reading_status === 'completed' ? 'selected' : ''}>読了</option>
                        </select>
                    </div>
                    <div class="edit-field">
                        <label>保管場所</label>
                        <div id="edit-storage-location-container"></div>
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>レーベル</label>
                        <div id="edit-label-container"></div>
                    </div>
                </div>
            </div>
            <div class="edit-section">
                <h3 class="edit-section-title">ファイル</h3>
                <div class="edit-epub-info" id="edit-epub-info">
                    ${
                        book.epub_file_hash
                            ? `<div class="edit-epub-current">
                                <span class="edit-epub-name">${escapeHtml(book.epub_file_name || book.epub_file_hash)}</span>
                                <a class="btn btn-xs btn-ghost" href="/epubs/${encodeURIComponent(book.epub_file_hash)}" target="_blank" rel="noopener">開く</a>
                            </div>`
                            : `<div class="edit-epub-empty">ファイル未登録</div>`
                    }
                </div>
                <div class="edit-epub-actions">
                    <label class="btn btn-xs btn-outline-success edit-epub-upload-label">
                        ${book.epub_file_hash ? "差し替え" : "アップロード"}
                        <input type="file" id="edit-epub-input" accept=".epub,.pdf,.zip,application/epub+zip,application/pdf,application/zip" hidden>
                    </label>
                    ${
                        book.epub_file_hash
                            ? `<button type="button" class="btn btn-xs btn-outline-danger" onclick="deleteEpub(${book.id})">削除</button>`
                            : ""
                    }
                </div>
            </div>
            <div class="edit-section" id="edit-tracks-section">
                <h3 class="edit-section-title">トラック</h3>
                <div id="edit-tracks-list"><p class="series-empty">読み込み中...</p></div>
            </div>
            <div class="edit-section">
                <h3 class="edit-section-title">アーティスト</h3>
                <div class="edit-author-list" id="edit-author-list">
                    ${authorList.map((a) => `
                        <div class="edit-author-item" data-author-id="${a.id}">
                            <input type="number" class="edit-author-order" value="${a.sort_order != null ? a.sort_order : 0}" min="0" step="1" onchange="updateAuthorOrder(${book.id}, ${a.id}, this.value)">
                            <div class="edit-author-info">
                                <div class="edit-author-name">${escapeHtml(a.name)}</div>
                                <div class="edit-author-meta">
                                    ${a.transcription ? `<span class="edit-author-yomi">${escapeHtml(a.transcription)}</span>` : ""}
                                    ${a.ndl_id ? `<span class="edit-author-ndl">NDL: ${escapeHtml(a.ndl_id)}</span>` : ""}
                                </div>
                            </div>
                            <button type="button" class="btn btn-xs btn-outline-danger" onclick="removeAuthorFromBook(${book.id}, ${a.id})">削除</button>
                        </div>
                    `).join("")}
                    ${authorList.length === 0 ? '<p class="series-empty">アーティストがいません</p>' : ""}
                </div>
                <div class="edit-author-add" id="edit-author-add-wrap">
                    <div id="edit-author-select-container"></div>
                    <button type="button" class="btn btn-xs btn-outline-success" onclick="addAuthorToBook(${book.id})">追加</button>
                </div>
            </div>
            <div class="edit-section">
                <h3 class="edit-section-title">シリーズ設定</h3>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>シリーズ</label>
                        <div id="edit-series-select-container"></div>
                    </div>
                    <div class="edit-field">
                        <label>シリーズ巻数</label>
                        <input type="number" name="series_number" value="${book.series_number != null ? book.series_number : ''}" min="1" step="1">
                    </div>
                </div>
                <div class="edit-field">
                    <label>大シリーズ</label>
                    <div id="edit-grand-series-select-container"></div>
                </div>
            </div>
            <div class="edit-section" id="edit-copies-section"></div>
            <div class="edit-actions">
                <a href="/" class="btn btn-md btn-ghost">戻る</a>
                <button type="submit" class="btn btn-md btn-primary">保存</button>
            </div>
        </form>
    `;

    const form = document.getElementById("edit-form");

    const seriesOpts = allSeries.map((s) => ({ value: s.id, label: s.name }));
    createSearchableSelect(document.getElementById("edit-series-select-container"), {
        options: seriesOpts,
        value: book.series_id,
        placeholder: "なし",
        onChange: (val) => {
            form.querySelector("input[name=series_id]").value = val != null ? val : "";
        },
    });

    const indirectGsIds = getBookIndirectGrandSeriesIds(book.id);

    const gsOpts = allGrandSeries.filter((gs) => !indirectGsIds.has(gs.id)).map((gs) => ({ value: gs.id, label: gs.name }));
    createSearchableSelect(document.getElementById("edit-grand-series-select-container"), {
        options: gsOpts,
        value: currentGrandSeries && !indirectGsIds.has(currentGrandSeries.id) ? currentGrandSeries.id : null,
        placeholder: indirectGsIds.size > 0 ? "シリーズ経由で所属中" : "なし",
        onChange: (val) => {
            form.querySelector("input[name=grand_series_id]").value = val != null ? val : "";
        },
    });

    const locOpts = allStorageLocations.map((l) => ({
        value: l.id,
        label: getStorageLocationPath(l.id),
    }));
    createSearchableSelect(document.getElementById("edit-storage-location-container"), {
        options: locOpts,
        value: book.storage_location_id,
        placeholder: "なし",
        onChange: (val) => {
            form.querySelector("input[name=storage_location_id]").value = val != null ? val : "";
        },
    });

    const labelOpts = allLabels.map((l) => ({ value: l.id, label: l.name }));
    createSearchableSelect(document.getElementById("edit-label-container"), {
        options: labelOpts,
        value: book.label_id,
        placeholder: "なし",
        onChange: (val) => {
            form.querySelector("input[name=label_id]").value = val != null ? val : "";
        },
    });

    const authorOpts = availableAuthors.map((a) => ({ value: a.id, label: a.name }));
    editAuthorSelect = createSearchableSelect(document.getElementById("edit-author-select-container"), {
        options: authorOpts,
        value: null,
        placeholder: "アーティストを追加...",
        clearable: false,
    });

    renderCopiesSection(book.id);
    loadAndRenderTracks(book.id);

    form.addEventListener("submit", (e) => saveBook(e, book.id));
}

async function updateAuthorOrder(bookId, authorId, value) {
    const sort_order = parseInt(value, 10);
    if (isNaN(sort_order)) return;
    try {
        await fetch(`/api/books/${bookId}/authors/${authorId}`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ sort_order }),
        });
    } catch {}
}

async function addAuthorToBook(bookId) {
    if (!editAuthorSelect) return;
    const authorId = editAuthorSelect.getValue();
    if (!authorId) return;

    try {
        const res = await fetch(`/api/books/${bookId}/authors/${authorId}`, { method: "POST" });
        if (res.ok) {
            await loadBooks();
            const res2 = await fetch("/api/authors");
            if (res2.ok) allAuthors = await res2.json();
            renderBookEdit(bookId);
        }
    } catch {}
}

async function removeAuthorFromBook(bookId, authorId) {
    const ok = await showConfirm({ message: "この作者を削除しますか？", okLabel: "削除" });
    if (!ok) return;

    try {
        const res = await fetch(`/api/books/${bookId}/authors/${authorId}`, { method: "DELETE" });
        if (res.ok) {
            await loadBooks();
            const res2 = await fetch("/api/authors");
            if (res2.ok) allAuthors = await res2.json();
            renderBookEdit(bookId);
        }
    } catch {}
}

async function saveBook(e, bookId) {
    e.preventDefault();

    if (editAuthorSelect) {
        const authorId = editAuthorSelect.getValue();
        if (authorId) {
            await fetch(`/api/books/${bookId}/authors/${authorId}`, { method: "POST" });
        }
    }

    const fd = new FormData(e.target);
    const body = {};
    for (const [key, val] of fd.entries()) {
        if (key === "series_id" || key === "grand_series_id" || key === "series_number" || key === "disc_count" || key === "storage_location_id" || key === "label_id") {
            body[key] = val === "" ? null : parseInt(val, 10);
        } else if (key === "isbn" || key === "isdn") {
            body[key] = val === "" ? null : val;
        } else {
            body[key] = val === "" ? null : val;
        }
    }
    try {
        const res = await fetch(`/api/books/${bookId}`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
        });
        if (res.ok) {
            window.location.href = "/";
        }
    } catch {}
}

// アップロード失敗時のメッセージを組み立てる。
// 413 でアプリ由来のエラー本文が無い場合は、リバースプロキシの上限超過とみなして
// 対処法を案内する (Dantalian 本体の初期値は cover 10MB / audio 100MB / file 500MB)。
async function describeUploadError(res, label) {
    let serverMsg = "";
    try {
        const err = await res.json();
        serverMsg = (err && err.error) || "";
    } catch {}
    if (res.status === 413 && !serverMsg) {
        return `${label}のアップロードに失敗しました (413 Payload Too Large): ` +
            `ファイルサイズが上限を超えています。` +
            `前面のリバースプロキシ (nginx: client_max_body_size / Caddy / Cloudflare 等) の上限をまず確認してください。` +
            `(参考: Dantalian 本体の初期値は file 500MB / audio 100MB / cover 10MB。設定画面で変更可能)`;
    }
    const detail = serverMsg ? `: ${serverMsg}` : "";
    return `${label}のアップロードに失敗しました (${res.status})${detail}`;
}

document.getElementById("edit-content").addEventListener("change", async (e) => {
    if (e.target.id !== "edit-cover-input" && e.target.id !== "edit-epub-input") return;
    const file = e.target.files[0];
    if (!file) return;

    const coverDisplay = document.getElementById("edit-cover-display");
    if (coverDisplay) coverDisplay.style.opacity = "0.5";

    const params = new URLSearchParams(window.location.search);
    const bid = parseInt(params.get("book"), 10);
    const cid = parseInt(params.get("cd"), 10);

    if (e.target.id === "edit-epub-input") {
        if (!bid) {
            e.target.value = "";
            if (coverDisplay) coverDisplay.style.opacity = "1";
            return;
        }
        const fd = new FormData();
        fd.append("file", file);
        try {
            const res = await fetch(`/api/books/${bid}/epub`, {
                method: "POST",
                body: fd,
            });
            if (!res.ok) {
                console.error("File upload failed:", res.status);
                alert(await describeUploadError(res, "ファイル"));
            }
            await loadBooks();
            renderBookEdit(bid);
        } catch (err) {
            console.error("File upload error:", err);
            alert("ファイルのアップロード中に通信エラーが発生しました");
        }
        e.target.value = "";
        if (coverDisplay) coverDisplay.style.opacity = "1";
        return;
    }

    const fd = new FormData();
    fd.append("cover", file);

    if (cid) {
        try {
            const res = await fetch(`/api/cds/${cid}/cover`, {
                method: "POST",
                body: fd,
            });
            if (!res.ok) {
                console.error("CD cover upload failed:", res.status);
                alert(await describeUploadError(res, "カバー画像"));
            }
            await loadCds();
            renderCdEdit(cid);
        } catch (err) {
            console.error("CD cover upload error:", err);
            alert("カバー画像のアップロード中に通信エラーが発生しました");
            if (coverDisplay) coverDisplay.style.opacity = "1";
        }
    } else if (bid) {
        try {
            const res = await fetch(`/api/books/${bid}/cover`, {
                method: "POST",
                body: fd,
            });
            if (!res.ok) {
                console.error("Book cover upload failed:", res.status);
                alert(await describeUploadError(res, "カバー画像"));
            }
            await loadBooks();
            renderBookEdit(bid);
        } catch (err) {
            console.error("Book cover upload error:", err);
            alert("カバー画像のアップロード中に通信エラーが発生しました");
            if (coverDisplay) coverDisplay.style.opacity = "1";
        }
    }
});

async function deleteCover(bookId) {
    const ok = await showConfirm({ message: "表紙画像を削除しますか？", okLabel: "削除" });
    if (!ok) return;

    try {
        const res = await fetch(`/api/books/${bookId}/cover`, { method: "DELETE" });
        if (res.ok) {
            await loadBooks();
            renderBookEdit(bookId);
        }
    } catch {}
}

async function deleteEpub(bookId) {
    const ok = await showConfirm({ message: "EPUBを削除しますか？", okLabel: "削除" });
    if (!ok) return;

    try {
        const res = await fetch(`/api/books/${bookId}/epub`, { method: "DELETE" });
        if (res.ok) {
            await loadBooks();
            renderBookEdit(bookId);
        }
    } catch {}
}

let currentCdTags = null; // 編集中CDの track_metadata 由来アルバムタグ合意(参考)

async function renderCdEdit(cdId) {
    const cd = (allCds || []).find((c) => c.id === cdId);
    if (!cd) {
        editContent.innerHTML = '<p class="empty-state">CDが見つかりません</p>';
        return;
    }

    // タグ由来のアルバム情報を取得(cd 側の編集値とは別に出し、仮入力/参考表示に使う)
    let tags = {};
    try {
        const tr = await fetch(`/api/cds/${cdId}/album-tags`);
        if (tr.ok) {
            const tj = await tr.json();
            if (tj && typeof tj === "object") tags = tj;
        }
    } catch (err) {
        console.error("loadCdAlbumTags failed:", err);
    }
    currentCdTags = tags;

    // 空欄への仮入力: cd 側の値がなければタグ値を初期値として表示(保存時に確定)
    const publisherVal = cd.publisher || tags.publisher || "";
    const labelVal = cd.label || tags.label || "";
    const publisherPh = cd.publisher && tags.publisher && cd.publisher !== tags.publisher
        ? `placeholder="タグ: ${escapeAttr(tags.publisher)}"` : "";
    const labelPh = cd.label && tags.label && cd.label !== tags.label
        ? `placeholder="タグ: ${escapeAttr(tags.label)}"` : "";

    editContent.innerHTML = `
        <h2>CD編集</h2>
        <div class="edit-header">
            <div class="edit-cover-wrap">
                <div class="edit-cover" id="edit-cover-display">
                    ${cd.cover_url
                        ? `<img class="book-cover" src="/images/${cd.cover_url}" alt="">`
                        : '<div class="book-cover-placeholder">No Image</div>'}
                </div>
                <div class="edit-cover-actions">
                    <label class="btn btn-xs btn-outline-success edit-cover-upload-label">
                        変更
                        <input type="file" id="edit-cover-input" accept="image/*" hidden>
                    </label>
                    ${cd.cover_url ? `<button type="button" class="btn btn-xs btn-outline-danger" onclick="deleteCdCover(${cd.id})">削除</button>` : ""}
                </div>
            </div>
            <div class="edit-header-info">
                ${cd.jan ? `<div class="edit-isbn">JAN: ${escapeHtml(cd.jan)}</div>` : ''}
                <div class="edit-isbn" style="font-size:0.75rem;color:var(--color-text-dim);">種別: ${escapeHtml(cd.media_type || 'cd')}</div>
            </div>
        </div>
        <form class="edit-form" id="edit-cd-form">
            <div class="edit-field">
                <label>タイトル <span class="edit-required">*</span></label>
                <input type="text" name="title" value="${escapeAttr(cd.title)}" required>
            </div>
            <div class="edit-row">
                <div class="edit-field">
                    <label>JAN</label>
                    <input type="text" name="jan" value="${escapeAttr(cd.jan || '')}">
                </div>
                <div class="edit-field">
                    <label>出版社</label>
                    <input type="text" name="publisher" value="${escapeAttr(publisherVal)}" ${publisherPh}>
                </div>
            </div>
            <div class="edit-row">
                <div class="edit-field">
                    <label>レーベル</label>
                    <input type="text" name="label" value="${escapeAttr(labelVal)}" ${labelPh}>
                </div>
                <div class="edit-field">
                    <label>品番</label>
                    <input type="text" name="catalog_number" value="${escapeAttr(cd.catalog_number || '')}">
                </div>
            </div>
            <div class="edit-row">
                <div class="edit-field">
                    <label>発売日</label>
                    <input type="text" name="publish_date" value="${escapeAttr(cd.publish_date || '')}">
                </div>
                <div class="edit-field">
                    <label>巻</label>
                    <input type="text" name="volume" value="${escapeAttr(cd.volume || '')}">
                </div>
            </div>
            <div class="edit-row">
                <div class="edit-field">
                    <label>ディスク枚数</label>
                    <input type="number" name="disc_count" value="${cd.disc_count != null ? cd.disc_count : ''}" min="1" step="1">
                </div>
            </div>
            <div class="edit-row">
                <div class="edit-field">
                    <label>種別</label>
                    <select name="media_type" class="form-input">
                        <option value="cd" ${(!cd.media_type || cd.media_type === 'cd') ? 'selected' : ''}>CD</option>
                        <option value="audiobook" ${cd.media_type === 'audiobook' ? 'selected' : ''}>オーディオブック</option>
                    </select>
                </div>
                <div class="edit-field">
                    <label>親書籍ID</label>
                    <input type="number" name="parent_book_id" value="${cd.parent_book_id != null ? cd.parent_book_id : ''}" min="1" step="1">
                </div>
            </div>
            <div class="edit-field">
                <label>説明</label>
                <textarea name="description" rows="4">${escapeHtml(cd.description || '')}</textarea>
            </div>
            <div class="edit-section" id="edit-cd-metadata-section">
                <h3 class="edit-section-title">アルバム情報 <span style="font-size:0.7rem;color:var(--color-text-dim)">(全トラックで共有・トラック編集のデフォルト値)</span></h3>
                <div id="edit-cd-metadata-fields"><p class="series-empty">読み込み中...</p></div>
            </div>
            <div class="edit-section" id="edit-tracks-section">
                <h3 class="edit-section-title">トラック</h3>
                <div id="edit-tracks-list"><p class="series-empty">読み込み中...</p></div>
            </div>
            <div class="edit-section">
                <h3 class="edit-section-title">アーティスト</h3>
                <div class="edit-author-list" id="edit-cd-author-list">
                    ${renderCdAuthorListHtml(cdId, cd.authors || [])}
                </div>
                ${(!(cd.authors || []).length && tags.album_artist) ? `
                <div class="edit-tag-hint">
                    タグ由来のアルバムアーティスト: <strong>${escapeHtml(tags.album_artist)}</strong>
                    <button type="button" class="btn btn-xs btn-outline-success" onclick="registerCdAlbumArtistFromTag(${cdId})">この名前で登録</button>
                </div>` : ""}
                <div class="edit-author-add">
                    <div id="edit-cd-author-select-container"></div>
                    <button type="button" class="btn btn-xs btn-outline-success" onclick="addAuthorToCd(${cdId})">追加</button>
                </div>
            </div>
            <div class="edit-section">
                <h3 class="edit-section-title">シリーズ設定</h3>
                <div class="edit-field">
                    <label>シリーズ</label>
                    <div id="edit-cd-series-select-container"></div>
                </div>
            </div>
            <div class="edit-actions">
                <a href="/" class="btn btn-md btn-ghost">戻る</a>
                <button type="submit" class="btn btn-md btn-primary">保存</button>
            </div>
        </form>
    `;

    const seriesOpts = allSeries.map((s) => ({ value: s.id, label: s.name }));
    editCdSeriesSelect = createSearchableSelect(document.getElementById("edit-cd-series-select-container"), {
        options: seriesOpts,
        value: cd.series_id,
        placeholder: "なし",
    });

    loadAndRenderCdTracks(cdId);
    loadAndRenderCdMetadata(cdId);

    const authorOpts = allAuthors.map((a) => ({ value: a.id, label: a.name }));
    editCdAuthorSelect = createSearchableSelect(document.getElementById("edit-cd-author-select-container"), {
        options: authorOpts,
        value: null,
        placeholder: "アーティストを追加...",
        clearable: false,
    });

    const form = document.getElementById("edit-cd-form");
    form.addEventListener("submit", (e) => saveCd(e, cdId));
}

async function loadAndRenderCdMetadata(cdId) {
    const container = document.getElementById("edit-cd-metadata-fields");
    if (!container) return;
    container.innerHTML = '<p class="series-empty">読み込み中...</p>';
    let meta = {};
    try {
        const res = await fetch(`/api/cds/${cdId}/metadata`);
        if (res.ok) {
            const data = await res.json();
            if (data && typeof data === "object") meta = data;
        }
    } catch (err) {
        console.error("loadAndRenderCdMetadata failed:", err);
    }
    const tags = currentCdTags || {};

    const fields = [
        { key: "year", label: "年", type: "number", min: 1000, max: 9999 },
        { key: "genre", label: "ジャンル", type: "text" },
        { key: "composer", label: "作曲", type: "text" },
        { key: "isrc", label: "ISRC", type: "text" },
    ];
    const html = fields.map((f) => {
        const raw = meta[f.key];
        const tagv = tags[f.key];
        const hasRaw = raw != null && String(raw) !== "";
        const val = hasRaw ? String(raw) : (tagv != null ? String(tagv) : "");
        const min = f.min != null ? `min="${f.min}"` : "";
        const max = f.max != null ? `max="${f.max}"` : "";
        const hint = hasRaw && tagv != null && String(tagv) !== "" && String(raw) !== String(tagv)
            ? `placeholder="タグ: ${escapeAttr(tagv)}"` : "";
        return `<div class="edit-field">
            <label>${escapeHtml(f.label)}</label>
            <input type="${f.type}" id="cd-meta-${f.key}" data-cd-meta-key="${f.key}" value="${escapeAttr(val)}" ${min} ${max} ${hint}>
        </div>`;
    });
    const rows = [];
    for (let i = 0; i < html.length; i += 2) {
        const pair = html.slice(i, i + 2).join("");
        rows.push(`<div class="edit-row">${pair}</div>`);
    }

    // タグ由来(参考): cd 側の編集欄とは別に、音声タグが何と言っていたかを表示する。
    const refRows = [
        ["アルバム名", tags.album],
        ["アルバムアーティスト", tags.album_artist],
        ["出版社", tags.publisher],
        ["レーベル", tags.label],
    ].filter(([, v]) => v != null && String(v) !== "");
    const refHtml = refRows.length > 0
        ? `<div class="edit-tag-ref">
               <div class="edit-tag-ref-title">音声タグ由来 (参考・編集は上の欄/基本情報/アーティスト欄で)</div>
               ${refRows.map(([l, v]) => `<div class="edit-tag-ref-row"><span class="edit-tag-ref-label">${escapeHtml(l)}</span><span class="edit-tag-ref-value">${escapeHtml(v)}</span></div>`).join("")}
           </div>`
        : "";

    container.innerHTML = rows.join("") + refHtml;
}

async function saveCdMetadata(cdId) {
    const container = document.getElementById("edit-cd-metadata-fields");
    if (!container) return;
    const body = {};
    container.querySelectorAll("input[data-cd-meta-key]").forEach((i) => {
        const k = i.dataset.cdMetaKey;
        const v = i.value.trim();
        if (v !== "") {
            if (i.type === "number") body[k] = parseInt(v, 10);
            else body[k] = v;
        }
    });
    try {
        const r = await fetch(`/api/cds/${cdId}/metadata`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
        });
        if (r.ok) {
            alert("アルバム情報を保存しました");
            if (typeof loadCds === "function") await loadCds();
        } else {
            const err = await r.json().catch(() => ({}));
            alert(`保存に失敗しました (HTTP ${r.status}): ${err.error || ""}`);
        }
    } catch (err) {
        console.error("saveCdMetadata error:", err);
        alert("通信エラーが発生しました");
    }
}

async function saveCd(e, cdId) {
    e.preventDefault();
    const fd = new FormData(e.target);
    const body = {};
    for (const [key, val] of fd.entries()) {
        if (key === "disc_count" || key === "parent_book_id") {
            body[key] = val === "" ? null : parseInt(val, 10);
        } else {
            body[key] = val === "" ? null : val;
        }
    }
    if (editCdSeriesSelect) {
        body.series_id = editCdSeriesSelect.getValue();
    }
    try {
        const res = await fetch(`/api/cds/${cdId}`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
        });
        if (res.ok) {
            window.location.href = "/";
        }
    } catch {}
}

function renderCdAuthorListHtml(cdId, authors) {
    if (!authors || authors.length === 0) {
        return '<p class="series-empty">アーティストがいません</p>';
    }
    return authors.map((a) => `
        <div class="edit-author-item" data-author-id="${a.id}">
            <input type="number" class="edit-author-order" value="${a.sort_order != null ? a.sort_order : 0}" min="0" step="1" onchange="updateCdAuthorOrder(${cdId}, ${a.id}, this.value)">
            <div class="edit-author-info">
                <div class="edit-author-name">${escapeHtml(a.name)}</div>
                <div class="edit-author-meta">
                    ${a.transcription ? `<span class="edit-author-yomi">${escapeHtml(a.transcription)}</span>` : ""}
                    ${a.ndl_id ? `<span class="edit-author-ndl">NDL: ${escapeHtml(a.ndl_id)}</span>` : ""}
                </div>
            </div>
            <button type="button" class="btn btn-xs btn-outline-danger" onclick="removeAuthorFromCd(${cdId}, ${a.id})">削除</button>
        </div>
    `).join("");
}

async function addAuthorToCd(cdId) {
    if (!editCdAuthorSelect) return;
    const authorId = editCdAuthorSelect.getValue();
    if (!authorId) return;
    try {
        const res = await fetch(`/api/cds/${cdId}/authors/${authorId}`, { method: "POST" });
        if (res.ok) {
            await loadCds();
            renderCdEdit(cdId);
        }
    } catch {}
}

// サーバ側の split_artist_names と概ね同じ区切りで名前を分割する。
function splitArtistNames(raw) {
    const out = [];
    for (const token of String(raw || "").split(/[,;/&\n]/)) {
        const cleaned = token
            .trim()
            .replace(/^(feat\.?|ft\.?|with|vs\.?)\s*/i, "")
            .trim();
        if (cleaned && !out.includes(cleaned)) out.push(cleaned);
    }
    return out;
}

// タグ由来のアルバムアーティストを作者として確保し、CD に紐付ける(空欄時のみ表示されるヒント用)。
async function registerCdAlbumArtistFromTag(cdId) {
    const names = splitArtistNames(currentCdTags && currentCdTags.album_artist);
    if (names.length === 0) return;
    try {
        const res = await fetch(`/api/cds/${cdId}/authors/from-names`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ names }),
        });
        if (res.ok) {
            const aRes = await fetch("/api/authors");
            if (aRes.ok) allAuthors = await aRes.json();
            await loadCds();
            renderCdEdit(cdId);
        } else {
            const err = await res.json().catch(() => ({}));
            alert(`登録に失敗しました (HTTP ${res.status}): ${err.error || ""}`);
        }
    } catch (err) {
        console.error("registerCdAlbumArtistFromTag error:", err);
        alert("通信エラーが発生しました");
    }
}

async function removeAuthorFromCd(cdId, authorId) {
    const ok = await showConfirm({ message: "このアーティストを削除しますか？", okLabel: "削除" });
    if (!ok) return;
    try {
        const res = await fetch(`/api/cds/${cdId}/authors/${authorId}`, { method: "DELETE" });
        if (res.ok) {
            await loadCds();
            renderCdEdit(cdId);
        }
    } catch {}
}

async function updateCdAuthorOrder(cdId, authorId, value) {
    const sort_order = parseInt(value, 10);
    if (isNaN(sort_order)) return;
    try {
        await fetch(`/api/cds/${cdId}/authors/${authorId}`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ sort_order }),
        });
    } catch {}
}

async function deleteCdCover(cdId) {
    const ok = await showConfirm({ message: "表紙画像を削除しますか？", okLabel: "削除" });
    if (!ok) return;
    try {
        const res = await fetch(`/api/cds/${cdId}/cover`, { method: "DELETE" });
        if (res.ok) {
            await loadCds();
            renderCdEdit(cdId);
        }
    } catch {}
}
