const registerForm = document.getElementById("register-form");
const isbnInput = document.getElementById("isbn-input");
const registerBtn = document.getElementById("register-btn");
const registerStatus = document.getElementById("register-status");
const bookGrid = document.getElementById("book-grid");
const bookCount = document.getElementById("book-count");
const detailOverlay = document.getElementById("detail-overlay");
const detailContent = document.getElementById("detail-content");

let allBooks = [];
let allSeries = [];

registerForm.addEventListener("submit", async (e) => {
    e.preventDefault();
    const isbn = isbnInput.value.trim().replace(/-/g, "");
    if (!isbn) return;

    registerBtn.disabled = true;
    registerStatus.textContent = "検索中...";
    registerStatus.className = "";

    try {
        const res = await fetch("/api/books", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ isbn }),
        });
        const data = await res.json();

        if (!res.ok) {
            registerStatus.textContent = data.error || "登録に失敗しました";
            registerStatus.className = "error";
            return;
        }

        const sourceLabel = data.source === "openbd" ? "OpenBD" : data.source === "ndl" ? "国立国会図書館" : "キャッシュ";
        registerStatus.textContent = `「${data.book.title}」を${sourceLabel}から登録しました`;
        registerStatus.className = "success";
        isbnInput.value = "";
        loadBooks();
    } catch (err) {
        registerStatus.textContent = "通信エラーが発生しました";
        registerStatus.className = "error";
    } finally {
        registerBtn.disabled = false;
    }
});

async function loadBooks() {
    try {
        const res = await fetch("/api/books");
        allBooks = await res.json();
        bookCount.textContent = `(${allBooks.length}冊)`;
        renderBooks();
    } catch {
        bookGrid.innerHTML = '<p class="empty-state">読み込みに失敗しました</p>';
    }
}

async function loadSeries() {
    try {
        const res = await fetch("/api/series");
        allSeries = await res.json();
    } catch {
        allSeries = [];
    }
}

function toggleSeriesManager() {
    const panel = document.getElementById("series-manager");
    panel.classList.toggle("hidden");
    if (!panel.classList.contains("hidden")) {
        renderSeriesManager();
    }
}

function renderSeriesManager() {
    const list = document.getElementById("series-list");
    if (allSeries.length === 0) {
        list.innerHTML = '<p style="color:#555;font-size:0.85rem;padding:0.5rem 0;">シリーズがありません</p>';
        return;
    }
    list.innerHTML = allSeries.map((s) => `
        <div class="series-list-item" id="series-item-${s.id}">
            <span class="series-list-name" ondblclick="startRenameSeries(${s.id}, '${escapeAttr(s.name)}')">${escapeHtml(s.name)}</span>
            <div class="series-list-actions">
                <button class="btn-rename" onclick="startRenameSeries(${s.id}, '${escapeAttr(s.name)}')">改名</button>
                <button class="btn-delete" onclick="deleteSeries(${s.id})">削除</button>
            </div>
        </div>
    `).join("");
}

async function createSeries() {
    const input = document.getElementById("new-series-name");
    const name = input.value.trim();
    if (!name) return;

    try {
        const res = await fetch("/api/series", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ name }),
        });
        if (res.ok) {
            input.value = "";
            await loadSeries();
            renderSeriesManager();
            loadBooks();
        }
    } catch {}
}

async function startRenameSeries(id, oldName) {
    const el = document.getElementById(`series-item-${id}`);
    if (!el) return;
    const nameEl = el.querySelector(".series-list-name");
    const actionsEl = el.querySelector(".series-list-actions");

    const input = document.createElement("input");
    input.className = "inline-edit-input";
    input.value = oldName;

    const save = async () => {
        const newName = input.value.trim();
        if (newName && newName !== oldName) {
            await fetch(`/api/series/${id}`, {
                method: "PUT",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ name: newName }),
            });
        }
        await loadSeries();
        renderSeriesManager();
        loadBooks();
    };

    const cancel = () => {
        nameEl.textContent = oldName;
        actionsEl.style.display = "";
    };

    input.addEventListener("keydown", (e) => {
        if (e.key === "Enter") save();
        if (e.key === "Escape") cancel();
    });
    input.addEventListener("blur", save);

    nameEl.textContent = "";
    nameEl.appendChild(input);
    actionsEl.style.display = "none";
    input.focus();
    input.select();
}

async function deleteSeries(id) {
    const s = allSeries.find((x) => x.id === id);
    if (!s) return;
    if (!confirm(`シリーズ「${s.name}」を削除しますか？\n所属している本はシリーズから外れます。`)) return;

    try {
        const res = await fetch(`/api/series/${id}`, { method: "DELETE" });
        if (res.ok) {
            await loadSeries();
            renderSeriesManager();
            loadBooks();
        }
    } catch {}
}

function groupByUserSeries(books) {
    const groups = {};
    const singles = [];

    for (const book of books) {
        if (book.series_id != null) {
            if (!groups[book.series_id]) {
                const s = allSeries.find((x) => x.id === book.series_id);
                groups[book.series_id] = {
                    series_id: book.series_id,
                    series_name: s ? s.name : `シリーズ ${book.series_id}`,
                    books: [],
                };
            }
            groups[book.series_id].books.push(book);
        } else {
            singles.push(book);
        }
    }

    return { groups, singles };
}

function renderBooks() {
    if (allBooks.length === 0) {
        bookGrid.innerHTML = '<p class="empty-state">ISBNで書籍を登録してください</p>';
        return;
    }

    const { groups, singles } = groupByUserSeries(allBooks);
    let html = "";

    const seriesEntries = Object.entries(groups).sort((a, b) =>
        b[1].books[0].id - a[1].books[0].id
    );

    for (const [, series] of seriesEntries) {
        series.books.sort((a, b) => a.id - b.id);
        const coversHtml = series.books.slice(0, 4).map((b) =>
            b.cover_url
                ? `<img class="book-cover" src="/images/${b.cover_url}" alt="" loading="lazy">`
                : '<div class="book-cover-placeholder">-</div>'
        ).join("");

        const volumesHtml = series.books.map((b) => `
            <div class="volume-item" onclick="showDetail(${b.id})">
                ${
                    b.cover_url
                        ? `<img class="book-cover" src="/images/${b.cover_url}" alt="" loading="lazy">`
                        : '<div class="book-cover-placeholder">-</div>'
                }
                <div class="volume-title">${escapeHtml(b.title)}</div>
            </div>`
        ).join("");

        html += `
        <div class="series-card">
            <div class="series-header" onclick="toggleSeries(this)">
                <div class="series-covers">${coversHtml}</div>
                <div class="series-info">
                    <div class="series-label">シリーズ</div>
                    <div class="series-title">${escapeHtml(series.series_name)}</div>
                    <div class="series-count">${series.books.length}冊</div>
                </div>
            </div>
            <div class="series-volumes">${volumesHtml}</div>
        </div>`;
    }

    for (const book of singles) {
        html += `
        <div class="book-card" onclick="showDetail(${book.id})">
            ${
                book.cover_url
                    ? `<img class="book-cover" src="/images/${book.cover_url}" alt="${escapeHtml(book.title)}" loading="lazy">`
                    : '<div class="book-cover-placeholder">No Image</div>'
            }
            <div class="book-info">
                <div class="book-title">${escapeHtml(book.title)}</div>
                ${book.author ? `<div class="book-author">${escapeHtml(book.author)}</div>` : ""}
                ${book.publisher ? `<div class="book-meta">${escapeHtml(book.publisher)}</div>` : ""}
            </div>
        </div>`;
    }

    bookGrid.innerHTML = html;
}

function toggleSeries(headerEl) {
    const volumes = headerEl.nextElementSibling;
    volumes.classList.toggle("open");
}

function showDetail(id) {
    const book = allBooks.find((b) => b.id === id);
    if (!book) return;

    const currentSeries = book.series_id != null
        ? allSeries.find((s) => s.id === book.series_id)
        : null;

    const metaParts = [];
    if (book.publisher) metaParts.push(`<div><span class="detail-meta-label">出版社</span>${escapeHtml(book.publisher)}</div>`);
    if (book.publish_date) metaParts.push(`<div><span class="detail-meta-label">出版日</span>${escapeHtml(book.publish_date)}</div>`);
    if (book.price) metaParts.push(`<div><span class="detail-meta-label">価格</span>${escapeHtml(book.price)}</div>`);
    if (book.extent) metaParts.push(`<div><span class="detail-meta-label">ページ数</span>${escapeHtml(book.extent)}</div>`);
    if (book.isbn) metaParts.push(`<div><span class="detail-meta-label">ISBN</span>${escapeHtml(book.isbn)}</div>`);

    const seriesOptions = allSeries
        .map((s) => `<option value="${s.id}" ${s.id === book.series_id ? "selected" : ""}>${escapeHtml(s.name)}</option>`)
        .join("");

    detailContent.innerHTML = `
        <div class="detail-header">
            <div class="detail-cover">
                ${
                    book.cover_url
                        ? `<img class="book-cover" src="/images/${book.cover_url}" alt="${escapeHtml(book.title)}">`
                        : '<div class="book-cover-placeholder">No Image</div>'
                }
            </div>
            <div class="detail-title-block">
                ${currentSeries ? `<div class="detail-series-name">${escapeHtml(currentSeries.name)}</div>` : ""}
                <div class="detail-title">${escapeHtml(book.title)}</div>
                ${book.author ? `<div class="detail-author">${escapeHtml(book.author)}</div>` : ""}
                <div class="detail-meta-list">${metaParts.join("")}</div>
            </div>
        </div>
        ${book.description ? `<div class="detail-description">${escapeHtml(book.description)}</div>` : ""}
        ${
            book.ndl_url
                ? `<a href="${escapeHtml(book.ndl_url)}" target="_blank" rel="noopener" style="color:#4ecca3;font-size:0.85rem;">国立国会図書館で見る</a>`
                : ""
        }
        <div class="detail-series-assign">
            <label>シリーズ</label>
            <select onchange="assignSeries(${book.id}, this.value)">
                <option value="">なし</option>
                ${seriesOptions}
            </select>
        </div>
        <div class="detail-actions">
            <button class="book-delete" onclick="deleteBook(${book.id})">削除</button>
        </div>
    `;

    detailOverlay.classList.remove("hidden");
}

async function assignSeries(bookId, value) {
    const seriesId = value === "" ? null : parseInt(value, 10);
    try {
        await fetch(`/api/books/${bookId}/series`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ series_id: seriesId }),
        });
        await loadBooks();
    } catch {}
}

function closeDetail(e) {
    if (e && e.target !== detailOverlay) return;
    detailOverlay.classList.add("hidden");
}

async function deleteBook(id) {
    if (!confirm("この書籍を削除しますか？")) return;
    try {
        const res = await fetch(`/api/books/${id}`, { method: "DELETE" });
        if (res.ok) {
            detailOverlay.classList.add("hidden");
            loadBooks();
        }
    } catch {}
}

function escapeHtml(text) {
    if (text == null) return "";
    const div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
}

function escapeAttr(text) {
    return text.replace(/'/g, "\\'").replace(/"/g, '&quot;');
}

document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") detailOverlay.classList.add("hidden");
});

(async () => {
    await loadSeries();
    loadBooks();
})();
