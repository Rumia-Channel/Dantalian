const editContent = document.getElementById("edit-content");

const params = new URLSearchParams(window.location.search);
const mode = params.get("mode");
const bookId = params.get("book");

(async () => {
    await loadBooks();
    await loadSeries();
    await loadGrandSeries();

    if (mode === "book" && bookId) {
        renderBookEdit(parseInt(bookId, 10));
    } else if (mode === "author") {
        const authorId = params.get("author");
        if (authorId) {
            renderAuthorEdit(parseInt(authorId, 10));
        } else {
            renderAuthorList();
        }
    } else {
        renderAuthorList();
    }
})();

function getUniqueAuthors() {
    const map = new Map();
    for (const book of allBooks) {
        if (!book.authors) continue;
        for (const a of book.authors) {
            if (!map.has(a.id)) map.set(a.id, a);
        }
    }
    return Array.from(map.values()).sort((a, b) => a.id - b.id);
}

function renderAuthorList() {
    const authors = getUniqueAuthors();
    editContent.innerHTML = `
        <h2>著者一覧 <span style="font-size:0.8rem;color:#888;">(${authors.length}件)</span></h2>
        <div class="edit-author-list">
            ${authors.map((a) => `
                <div class="edit-author-item" id="author-item-${a.id}">
                    <div class="edit-author-info">
                        <div class="edit-author-name">${escapeHtml(a.name)}</div>
                        <div class="edit-author-meta">
                            ${a.ndl_id ? `<span class="edit-author-ndl">NDL: ${escapeHtml(a.ndl_id)}</span>` : ""}
                            ${a.transcription ? `<span class="edit-author-yomi">${escapeHtml(a.transcription)}</span>` : ""}
                        </div>
                    </div>
                    <button class="btn-edit-sm" onclick="renderAuthorEdit(${a.id})">編集</button>
                </div>
            `).join("")}
            ${authors.length === 0 ? '<p class="series-empty">著者がいません</p>' : ""}
        </div>
    `;
}

function renderAuthorEdit(id) {
    const author = getUniqueAuthors().find((a) => a.id === id);
    if (!author) {
        editContent.innerHTML = '<p class="empty-state">著者が見つかりません</p>';
        return;
    }

    editContent.innerHTML = `
        <h2>著者情報編集</h2>
        <div class="edit-author-edit-header">
            <span class="edit-author-id">ID: ${author.id}</span>
            ${author.ndl_id ? `<span class="edit-author-ndl">NDL: ${escapeHtml(author.ndl_id)}</span>` : ""}
        </div>
        <form class="edit-form" onsubmit="saveAuthor(event, ${author.id})">
            <div class="edit-field">
                <label>名前 <span class="edit-required">*</span></label>
                <input type="text" name="name" value="${escapeAttr(author.name)}" required>
            </div>
            <div class="edit-field">
                <label>よみ</label>
                <input type="text" name="transcription" value="${escapeAttr(author.transcription || '')}">
            </div>
            <div class="edit-field">
                <label>NDL ID</label>
                <input type="text" name="ndl_id" value="${escapeAttr(author.ndl_id || '')}">
            </div>
            <div class="edit-actions">
                <button type="button" class="btn-cancel" onclick="renderAuthorList()">一覧に戻る</button>
                <button type="submit">保存</button>
            </div>
        </form>
    `;
}

function renderBookEdit(id) {
    const book = allBooks.find((b) => b.id === id);
    if (!book) {
        editContent.innerHTML = '<p class="empty-state">書籍が見つかりません</p>';
        return;
    }

    const authorList = book.authors || [];

    editContent.innerHTML = `
        <h2>書籍情報編集</h2>
        <div class="edit-header">
            <div class="edit-cover">
                ${book.cover_url
                    ? `<img class="book-cover" src="/images/${book.cover_url}" alt="">`
                    : '<div class="book-cover-placeholder">No Image</div>'}
            </div>
            <div class="edit-header-info">
                <div class="edit-isbn">${escapeHtml(book.isbn)}</div>
                <div class="edit-authors">
                    ${authorList.map((a) => `
                        <span class="edit-author-tag" onclick="renderAuthorEdit(${a.id})">${escapeHtml(a.name)}</span>
                    `).join("")}
                </div>
            </div>
        </div>
        <form class="edit-form" onsubmit="saveBook(event, ${book.id})">
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
                    <label>出版社</label>
                    <input type="text" name="publisher" value="${escapeAttr(book.publisher || '')}">
                </div>
                <div class="edit-field">
                    <label>出版日</label>
                    <input type="text" name="publish_date" value="${escapeAttr(book.publish_date || '')}">
                </div>
            </div>
            <div class="edit-row">
                <div class="edit-field">
                    <label>価格</label>
                    <input type="text" name="price" value="${escapeAttr(book.price || '')}">
                </div>
                <div class="edit-field">
                    <label>ページ数</label>
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
            <div class="edit-row">
                <div class="edit-field">
                    <label>JPNO</label>
                    <input type="text" name="jpno" value="${escapeAttr(book.jpno || '')}">
                </div>
                <div class="edit-field">
                    <label>NDL URL</label>
                    <input type="text" name="ndl_url" value="${escapeAttr(book.ndl_url || '')}">
                </div>
            </div>
            <div class="edit-field">
                <label>説明</label>
                <textarea name="description" rows="6">${escapeHtml(book.description || '')}</textarea>
            </div>
            <div class="edit-actions">
                <button type="button" class="btn-cancel" onclick="renderAuthorList()">一覧に戻る</button>
                <button type="submit">保存</button>
            </div>
        </form>
    `;
}

async function saveBook(e, bookId) {
    e.preventDefault();
    const fd = new FormData(e.target);
    const body = {};
    for (const [key, val] of fd.entries()) {
        body[key] = val === "" ? null : val;
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

async function saveAuthor(e, authorId) {
    e.preventDefault();
    const fd = new FormData(e.target);
    const body = {};
    for (const [key, val] of fd.entries()) {
        body[key] = val === "" ? null : val;
    }
    try {
        const res = await fetch(`/api/authors/${authorId}`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
        });
        if (res.ok) {
            await loadBooks();
            renderAuthorEdit(authorId);
        }
    } catch {}
}
