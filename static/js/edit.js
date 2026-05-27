const editContent = document.getElementById("edit-content");

const params = new URLSearchParams(window.location.search);
const bookId = params.get("book");

let allAuthors = [];
let allBorrowers = [];
let editAuthorSelect = null;

(async () => {
    await loadBooks();
    await loadSeries();
    await loadGrandSeries();

    const [aRes, bRes] = await Promise.all([
        fetch("/api/authors"),
        fetch("/api/borrowers"),
    ]);
    if (aRes.ok) allAuthors = await aRes.json();
    if (bRes.ok) allBorrowers = await bRes.json();

    if (bookId) {
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
                <label>シリーズ名(NDL)</label>
                <input type="text" name="series_title" value="${escapeAttr(book.series_title || '')}">
            </div>
            <div class="edit-field">
                <label>シリーズ名(よみ)</label>
                <input type="text" name="series_title_transcription" value="${escapeAttr(book.series_title_transcription || '')}">
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
            <div class="edit-section" id="edit-tracks-section">
                <h3 class="edit-section-title">トラック</h3>
                <div id="edit-tracks-list"><p class="series-empty">読み込み中...</p></div>
            </div>
            <div class="edit-section">
                <h3 class="edit-section-title">作者</h3>
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
                    ${authorList.length === 0 ? '<p class="series-empty">作者がいません</p>' : ""}
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

    const authorOpts = availableAuthors.map((a) => ({ value: a.id, label: a.name }));
    editAuthorSelect = createSearchableSelect(document.getElementById("edit-author-select-container"), {
        options: authorOpts,
        value: null,
        placeholder: "作者を追加...",
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
        if (key === "series_id" || key === "grand_series_id" || key === "series_number" || key === "disc_count") {
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

document.getElementById("edit-content").addEventListener("change", async (e) => {
    if (e.target.id !== "edit-cover-input") return;
    const file = e.target.files[0];
    if (!file) return;

    const bid = parseInt(new URLSearchParams(window.location.search).get("book"), 10);
    if (!bid) return;

    const fd = new FormData();
    fd.append("cover", file);

    try {
        const res = await fetch(`/api/books/${bid}/cover`, {
            method: "POST",
            body: fd,
        });
        if (res.ok) {
            await loadBooks();
            renderBookEdit(bid);
        }
    } catch {}
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

async function renderCopiesSection(bookId) {
    const section = document.getElementById("edit-copies-section");
    if (!section) return;

    let copies = [];
    try {
        const res = await fetch(`/api/books/${bookId}/copies`);
        if (res.ok) copies = await res.json();
    } catch {}

    let html = `<h3 class="edit-section-title">所蔵管理 (${copies.length}件)</h3>`;

    copies.forEach((c) => {
        const isLent = !!c.lent_to;
        html += `
        <div class="edit-copy-item${isLent ? ' copy-lent' : ''}">
            <div class="edit-copy-info">
                <div class="edit-copy-main">
                    <span class="copy-type-icon">${c.copy_type === 'ebook' ? 'smartphone' : 'menu_book'}</span>
                    <span class="edit-copy-location">${c.location ? escapeHtml(c.location) : '<span class="copy-no-location">場所未設定</span>'}</span>
                    ${isLent
                        ? `<span class="copy-lent-badge">貸出中: ${escapeHtml(c.lent_to)}</span>`
                        : '<span class="copy-available-badge">所持</span>'}
                    ${c.due_date ? `<span class="copy-due-date">返却予定: ${escapeHtml(c.due_date)}</span>` : ''}
                </div>
                ${c.notes ? `<div class="edit-copy-notes">${escapeHtml(c.notes)}</div>` : ''}
            </div>
            <div class="edit-copy-actions">
                ${isLent
                    ? `<button type="button" class="btn btn-xs btn-outline-success" onclick="returnCopy(${c.id}, ${bookId})">返却</button>`
                    : `<button type="button" class="btn btn-xs btn-outline-warning" onclick="showLendForm(${c.id}, ${bookId})">貸出</button>`}
                <button type="button" class="btn btn-xs btn-ghost" onclick="editCopyDialog(${c.id}, ${c.copy_type}, ${JSON.stringify(c.location).replace(/"/g, '&quot;')}, ${JSON.stringify(c.notes).replace(/"/g, '&quot;')}, ${bookId})">編集</button>
                <button type="button" class="btn btn-xs btn-outline-danger" onclick="deleteCopy(${c.id}, ${bookId})">削除</button>
            </div>
        </div>`;
    });

    html += `
        <div class="edit-copy-add" id="edit-copy-add">
            <button type="button" class="btn btn-xs btn-outline-success" onclick="addCopy(${bookId})">+ 所蔵を追加</button>
        </div>
    `;

    html += `<div id="edit-lend-form-container"></div>`;

    section.innerHTML = html;
}

async function addCopy(bookId) {
    try {
        const res = await fetch(`/api/books/${bookId}/copies`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ copy_type: "physical" }),
        });
        if (res.ok) {
            await loadBooks();
            renderCopiesSection(bookId);
        }
    } catch {}
}

async function editCopyDialog(copyId, copyType, location, notes, bookId) {
    const newLocation = prompt("場所", location || "");
    if (newLocation === null) return;
    const newNotes = prompt("メモ", notes || "");
    if (newNotes === null) return;

    try {
        const res = await fetch(`/api/copies/${copyId}`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
                location: newLocation || null,
                notes: newNotes || null,
            }),
        });
        if (res.ok) {
            await loadBooks();
            renderCopiesSection(bookId);
        }
    } catch {}
}

async function deleteCopy(copyId, bookId) {
    const ok = await showConfirm({ message: "この所蔵を削除しますか？", okLabel: "削除" });
    if (!ok) return;

    try {
        const res = await fetch(`/api/copies/${copyId}`, { method: "DELETE" });
        if (res.ok) {
            await loadBooks();
            renderCopiesSection(bookId);
        }
    } catch {}
}

function showLendForm(copyId, bookId) {
    const container = document.getElementById("edit-lend-form-container");
    if (!container) return;

    const borrowerOpts = allBorrowers.map((b) => `<option value="${b.id}">${escapeHtml(b.name)}</option>`).join("");

    container.innerHTML = `
        <div class="edit-lend-form">
            <div class="edit-lend-title">貸出</div>
            <div class="edit-lend-fields">
                <select id="lend-borrower-select">
                    <option value="">借り手を選択...</option>
                    ${borrowerOpts}
                </select>
                <input type="date" id="lend-due-date" placeholder="返却予定日">
                <button type="button" class="btn btn-sm btn-primary" onclick="doLendCopy(${copyId}, ${bookId})">貸出実行</button>
                <button type="button" class="btn btn-sm btn-ghost" onclick="cancelLend()">キャンセル</button>
            </div>
        </div>
    `;
}

function cancelLend() {
    const container = document.getElementById("edit-lend-form-container");
    if (container) container.innerHTML = "";
}

async function doLendCopy(copyId, bookId) {
    const borrowerId = parseInt(document.getElementById("lend-borrower-select").value, 10);
    if (!borrowerId) return;

    const dueDateInput = document.getElementById("lend-due-date");
    const dueDate = dueDateInput ? dueDateInput.value || null : null;

    try {
        const res = await fetch(`/api/copies/${copyId}/lend`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ borrower_id: borrowerId, due_date: dueDate }),
        });
        if (res.ok) {
            await loadBooks();
            renderCopiesSection(bookId);
        }
    } catch {}
}

async function returnCopy(copyId, bookId) {
    const ok = await showConfirm({ message: "返却しますか？", okLabel: "返却" });
    if (!ok) return;

    try {
        const res = await fetch(`/api/copies/${copyId}/return`, { method: "POST" });
        if (res.ok) {
            await loadBooks();
            renderCopiesSection(bookId);
        }
    } catch {}
}

async function loadAndRenderTracks(bookId) {
    const list = document.getElementById("edit-tracks-list");
    if (!list) return;

    try {
        const res = await fetch(`/api/books/${bookId}/tracks`);
        if (!res.ok) { list.innerHTML = "<p class='series-empty'>トラックなし</p>"; return; }
        const tracks = await res.json();
        if (tracks.length === 0) { list.innerHTML = "<p class='series-empty'>トラックなし</p>"; return; }

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
                               <button class="btn btn-xs btn-outline-danger" onclick="deleteTrackAudio(${bookId},${t.id})">削除</button>`
                            : `<label class="btn btn-xs btn-outline-success" style="cursor:pointer">
                                   アップロード
                                   <input type="file" accept="audio/*" hidden onchange="uploadTrackAudio(${bookId},${t.id},this)">
                               </label>`}
                    </div>
                </div>
            `).join("");
        }
        list.innerHTML = html;
    } catch {
        list.innerHTML = "<p class='series-empty'>トラック読み込みエラー</p>";
    }
}

async function uploadTrackAudio(bookId, trackId, input) {
    const file = input.files[0];
    if (!file) return;
    const fd = new FormData();
    fd.append("audio", file);
    try {
        const res = await fetch(`/api/books/${bookId}/tracks/${trackId}/audio`, { method: "POST", body: fd });
        if (res.ok) loadAndRenderTracks(bookId);
    } catch {}
}

async function deleteTrackAudio(bookId, trackId) {
    if (!await showConfirm({ message: "音声ファイルを削除しますか？", okLabel: "削除" })) return;
    try {
        const res = await fetch(`/api/books/${bookId}/tracks/${trackId}/audio`, { method: "DELETE" });
        if (res.ok) loadAndRenderTracks(bookId);
    } catch {}
}
