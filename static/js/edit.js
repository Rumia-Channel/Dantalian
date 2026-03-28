const editContent = document.getElementById("edit-content");

const params = new URLSearchParams(window.location.search);
const bookId = params.get("book");

(async () => {
    await loadBooks();

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
                        <span class="edit-author-tag" onclick="location.href='/authors/?edit=${a.id}'">${escapeHtml(a.name)}</span>
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
                <a href="/" class="btn btn-md btn-ghost">戻る</a>
                <button type="submit" class="btn btn-md btn-primary">保存</button>
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
