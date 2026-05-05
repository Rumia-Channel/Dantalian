let currentView = "author";
let currentSort = localStorage.getItem("tsukuyomi_sort") || "id";
const authorBookGrid = document.getElementById("book-grid");
const bookSearchInput = document.getElementById("book-search-input");
const bookSearchClear = document.getElementById("book-search-clear");
let currentSearchQuery = localStorage.getItem("tsukuyomi_book_search") || "";

function loadCollapsedAuthorGroups() {
    try {
        const saved = JSON.parse(localStorage.getItem("tsukuyomi_collapsed_author_groups") || "[]");
        return Array.isArray(saved) ? saved : [];
    } catch {
        return [];
    }
}

const collapsedAuthorGroups = new Set(loadCollapsedAuthorGroups());

function saveCollapsedAuthorGroups() {
    localStorage.setItem("tsukuyomi_collapsed_author_groups", JSON.stringify([...collapsedAuthorGroups]));
}

document.querySelectorAll(".view-tab").forEach((tab) => {
    tab.addEventListener("click", () => {
        document.querySelectorAll(".view-tab").forEach((t) => t.classList.remove("active"));
        tab.classList.add("active");
        currentView = tab.dataset.view;
        renderBooks();
    });
});

document.getElementById("sort-buttons").addEventListener("click", (e) => {
    const btn = e.target.closest(".width-btn");
    if (!btn) return;
    currentSort = btn.dataset.sort;
    localStorage.setItem("tsukuyomi_sort", currentSort);
    document.querySelectorAll("#sort-buttons .width-btn").forEach((b) => {
        b.classList.toggle("active", b.dataset.sort === currentSort);
    });
    renderBooks();
});

bookSearchInput.value = currentSearchQuery;
bookSearchClear.classList.toggle("hidden", currentSearchQuery.length === 0);

bookSearchInput.addEventListener("input", () => {
    currentSearchQuery = bookSearchInput.value.trim();
    localStorage.setItem("tsukuyomi_book_search", currentSearchQuery);
    bookSearchClear.classList.toggle("hidden", currentSearchQuery.length === 0);
    renderBooks();
});

bookSearchClear.addEventListener("click", () => {
    bookSearchInput.value = "";
    currentSearchQuery = "";
    localStorage.removeItem("tsukuyomi_book_search");
    bookSearchClear.classList.add("hidden");
    renderBooks();
    bookSearchInput.focus();
});

authorBookGrid.addEventListener("click", (e) => {
    const btn = e.target.closest(".author-group-header");
    if (!btn || !authorBookGrid.contains(btn)) return;

    const group = btn.closest(".author-group");
    const groupKey = group?.dataset.authorGroup;
    if (!group || !groupKey) return;

    e.preventDefault();
    e.stopPropagation();

    const isCollapsed = group.classList.toggle("author-group--collapsed");
    btn.setAttribute("aria-expanded", String(!isCollapsed));

    if (isCollapsed) {
        collapsedAuthorGroups.add(groupKey);
    } else {
        collapsedAuthorGroups.delete(groupKey);
    }
    saveCollapsedAuthorGroups();
});

function getPrimaryAuthor(book) {
    if (!book.authors || book.authors.length === 0) return null;
    return book.authors.reduce((best, a) =>
        (a.sort_order < best.sort_order) ? a : best
    );
}

function normalizeSearchText(value) {
    return String(value || "")
        .toLowerCase()
        .normalize("NFKC")
        .replace(/[ぁ-ゖ]/g, (ch) => String.fromCharCode(ch.charCodeAt(0) + 0x60))
        .replace(/\s+/g, "");
}

function getBookSearchText(book) {
    const series = book.series_id != null ? allSeries.find((s) => s.id === book.series_id) : null;
    const grandSeriesNames = allGrandSeries
        .filter((gs) => {
            if (gs.items.some((it) => it.item_type === "book" && it.item_id === book.id)) return true;
            return book.series_id != null && gs.items.some((it) => it.item_type === "series" && it.item_id === book.series_id);
        })
        .map((gs) => gs.name);
    return [
        book.title,
        book.title_transcription,
        book.alternative,
        book.alternative_transcription,
        book.volume,
        book.volume_transcription,
        book.publisher,
        book.publish_date,
        book.isbn,
        book.isdn,
        book.series_title,
        book.series_title_transcription,
        book.jpno,
        book.ndl_url,
        series?.name,
        ...grandSeriesNames,
        ...(book.authors || []).flatMap((a) => [a.name, a.transcription, a.ndl_id]),
    ].map(normalizeSearchText).join(" ");
}

function getFilteredBooks() {
    const query = normalizeSearchText(currentSearchQuery);
    if (!query) return allBooks;
    return allBooks.filter((book) => getBookSearchText(book).includes(query));
}

function getGrandSeriesBookIds(gs) {
    const ids = new Set();
    for (const item of gs.items) {
        if (item.item_type === "book") {
            ids.add(item.item_id);
        } else if (item.item_type === "series") {
            const seriesBooks = allBooks.filter((b) => b.series_id === item.item_id);
            for (const b of seriesBooks) ids.add(b.id);
        }
    }
    return ids;
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

function parseVolumeSortKey(vol) {
    if (!vol) return [4, 0];
    const prefixes = [
        ["上", 1],
        ["前", 1],
        ["中", 2],
        ["下", 3],
        ["後", 3],
    ];
    for (const [p, base] of prefixes) {
        if (vol.startsWith(p)) {
            const rest = vol.slice(p.length);
            const num = rest ? parseInt(rest, 10) : 0;
            return [base, isNaN(num) ? 0 : num];
        }
    }
    const num = parseInt(vol, 10);
    return [4, isNaN(num) ? 0 : num];
}

function sortBookList(books) {
    return [...books].sort((a, b) => {
        const snA = a.series_number != null ? a.series_number : Infinity;
        const snB = b.series_number != null ? b.series_number : Infinity;

        if (snA !== Infinity || snB !== Infinity) {
            if (snA !== snB) return snA - snB;
            const va = parseVolumeSortKey(a.volume);
            const vb = parseVolumeSortKey(b.volume);
            if (va[0] !== vb[0] || va[1] !== vb[1]) return va[0] - vb[0] || va[1] - vb[1];
            return b.id - a.id;
        }

        let cmp = 0;
        if (currentSort === "title") {
            cmp = a.title.localeCompare(b.title, "ja");
        } else if (currentSort === "publish_date") {
            const da = a.publish_date || "";
            const db = b.publish_date || "";
            cmp = db.localeCompare(da);
        } else {
            cmp = b.id - a.id;
        }
        if (cmp !== 0) return cmp;
        const va = parseVolumeSortKey(a.volume);
        const vb = parseVolumeSortKey(b.volume);
        if (va[0] !== vb[0] || va[1] !== vb[1]) return va[0] - vb[0] || va[1] - vb[1];
        return b.id - a.id;
    });
}

function getMaxId(books) {
    return books.length > 0 ? Math.max(...books.map((b) => b.id)) : 0;
}

function getLatestDate(books) {
    const dates = books.map((b) => b.publish_date || "").filter((d) => d);
    if (dates.length === 0) return "";
    return [...dates].sort().reverse()[0];
}

function renderBookCard(book) {
    return `
    <div class="book-card" onclick="showDetail(${book.id})">
        ${
            book.cover_url
                ? `<img class="book-cover" src="/images/${book.cover_url}" alt="${escapeHtml(book.title)}" loading="lazy">`
                : '<div class="book-cover-placeholder">No Image</div>'
        }
        <div class="book-info">
            <div class="book-title">${escapeHtml(book.title)}</div>
            ${book.authors && book.authors.length > 0 ? `<div class="book-author">${book.authors.map((a) => escapeHtml(a.name)).join(", ")}</div>` : ""}
            ${book.publisher ? `<div class="book-meta">${escapeHtml(book.publisher)}</div>` : ""}
        </div>
    </div>`;
}

function renderSeriesCard(series, books) {
    const coversHtml = books.slice(0, 8).map((b) =>
        b.cover_url
            ? `<img class="book-cover" src="/images/${b.cover_url}" alt="" loading="lazy">`
            : '<div class="book-cover-placeholder"></div>'
    ).join("");

    return `
    <div class="series-card" onclick="showSeriesModal(${series.series_id})">
        <div class="series-covers">${coversHtml}</div>
        <div class="series-info">
            <div class="series-label">シリーズ</div>
            <div class="series-title">${escapeHtml(series.series_name)}</div>
            <div class="series-count">${books.length}冊</div>
        </div>
    </div>`;
}

function renderGrandSeriesCard(gs, books) {
    const coversHtml = books.slice(0, 12).map((b) =>
        b.cover_url
            ? `<img class="book-cover" src="/images/${b.cover_url}" alt="" loading="lazy">`
            : '<div class="book-cover-placeholder"></div>'
    ).join("");

    const seriesCount = gs.items.filter((it) => it.item_type === "series").length;
    const directBookCount = gs.items.filter((it) => it.item_type === "book").length;
    const subInfo = [];
    if (seriesCount > 0) subInfo.push(`${seriesCount}シリーズ`);
    if (directBookCount > 0) subInfo.push(`${directBookCount}冊`);
    const subInfoStr = subInfo.join(" + ");

    return `
    <div class="grand-series-card" onclick="showGrandSeriesModal(${gs.id})">
        <div class="series-covers grand-series-covers">${coversHtml}</div>
        <div class="series-info">
            <div class="series-label grand-label">大シリーズ</div>
            <div class="series-title">${escapeHtml(gs.name)}</div>
            <div class="series-count">${books.length}冊${subInfoStr ? ` (${subInfoStr})` : ""}</div>
        </div>
    </div>`;
}

function buildGridHtml(books) {
    const seriesIdsInGrand = new Set();
    const bookIdsInGrand = new Set();
    for (const gs of allGrandSeries) {
        for (const item of gs.items) {
            if (item.item_type === "series") seriesIdsInGrand.add(item.item_id);
            if (item.item_type === "book") bookIdsInGrand.add(item.item_id);
        }
    }

    const items = [];

    for (const gs of allGrandSeries) {
        const gsBookIds = getGrandSeriesBookIds(gs);
        const gsBooks = books.filter((b) => gsBookIds.has(b.id));
        if (gsBooks.length === 0) continue;
        items.push({ type: "grand_series", gs, books: gsBooks });
    }

    const { groups, singles } = groupByUserSeries(books);

    for (const [sid, series] of Object.entries(groups)) {
        if (seriesIdsInGrand.has(parseInt(sid, 10))) continue;
        items.push({ type: "series", series, books: series.books });
    }

    for (const book of singles) {
        if (bookIdsInGrand.has(book.id)) continue;
        items.push({ type: "book", book, books: [book] });
    }

    for (const item of items) {
        item._sorted = sortBookList(item.books);
    }

    function getItemVolumeKey(item) {
        if (item._sorted.length > 0) return parseVolumeSortKey(item._sorted[0].volume);
        return [99, 0];
    }

    items.sort((a, b) => {
        if (currentSort === "title") {
            const nameA = a.type === "grand_series" ? a.gs.name : a.type === "series" ? a.series.series_name : a.book.title;
            const nameB = b.type === "grand_series" ? b.gs.name : b.type === "series" ? b.series.series_name : b.book.title;
            const cmp = nameA.localeCompare(nameB, "ja");
            if (cmp !== 0) return cmp;
            const vkA = getItemVolumeKey(a);
            const vkB = getItemVolumeKey(b);
            if (vkA[0] !== vkB[0] || vkA[1] !== vkB[1]) return vkA[0] - vkB[0] || vkA[1] - vkB[1];
        } else if (currentSort === "publish_date") {
            const da = getLatestDate(a.books);
            const db = getLatestDate(b.books);
            if (da !== db) return db.localeCompare(da);
        }
        return getMaxId(b.books) - getMaxId(a.books);
    });

    let html = "";
    for (const item of items) {
        const sortedBooks = item._sorted;
        if (item.type === "grand_series") {
            html += renderGrandSeriesCard(item.gs, sortedBooks);
        } else if (item.type === "series") {
            html += renderSeriesCard(item.series, sortedBooks);
        } else {
            html += renderBookCard(item.book);
        }
    }
    return html;
}

function renderBooksByAuthor(books) {
    const authorMap = new Map();

    for (const book of books) {
        const primary = getPrimaryAuthor(book);
        if (!primary) {
            if (!authorMap.has("__none__")) authorMap.set("__none__", { author: null, books: [] });
            authorMap.get("__none__").books.push(book);
            continue;
        }
        if (!authorMap.has(primary.id)) {
            authorMap.set(primary.id, { author: primary, books: [] });
        }
        authorMap.get(primary.id).books.push(book);
    }

    const entries = [...authorMap.entries()].sort((a, b) => {
        if (a[0] === "__none__") return 1;
        if (b[0] === "__none__") return -1;
        return a[1].author.name.localeCompare(b[1].author.name, "ja");
    });

    let html = "";

    for (const [key, { author, books }] of entries) {
        const groupKey = String(key);
        const isCollapsed = currentSearchQuery.length === 0 && collapsedAuthorGroups.has(groupKey);
        const headerHtml = author
            ? `<button type="button" class="author-group-header" aria-expanded="${!isCollapsed}">
                    <span class="author-group-name">${escapeHtml(author.name)}</span>
                    <span class="author-group-count">${books.length}冊</span>
                </button>`
            : `<button type="button" class="author-group-header author-group-header--none" aria-expanded="${!isCollapsed}">
                    <span class="author-group-name">作者未設定</span>
                    <span class="author-group-count">${books.length}冊</span>
                </button>`;

        html += `<div class="author-group${isCollapsed ? " author-group--collapsed" : ""}" data-author-group="${escapeHtml(groupKey)}">${headerHtml}<div class="author-group-grid">`;
        html += buildGridHtml(books);
        html += `</div></div>`;
    }

    return html;
}

function renderBooks() {
    if (allBooks.length === 0) {
        bookGrid.innerHTML = '<p class="empty-state">ISBNで書籍を登録してください</p>';
        bookCount.textContent = "(0冊)";
        return;
    }

    const books = getFilteredBooks();
    bookCount.textContent = currentSearchQuery
        ? `(${books.length} / ${allBooks.length}冊)`
        : `(${allBooks.length}冊)`;

    if (books.length === 0) {
        bookGrid.className = "";
        bookGrid.innerHTML = '<p class="empty-state">該当する書籍がありません</p>';
        return;
    }

    if (currentView === "author") {
        bookGrid.className = "book-grid-author";
        bookGrid.innerHTML = renderBooksByAuthor(books);
    } else {
        bookGrid.className = "";
        bookGrid.id = "book-grid";
        bookGrid.innerHTML = buildGridHtml(books);
    }
}

(function initSortButtons() {
    document.querySelectorAll("#sort-buttons .width-btn").forEach((btn) => {
        btn.classList.toggle("active", btn.dataset.sort === currentSort);
    });
})();
