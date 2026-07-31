let currentView = localStorage.getItem("dantalian_view") || "author";
let currentSort = localStorage.getItem("dantalian_sort") || "id";
let currentTypeFilter = localStorage.getItem("dantalian_type_filter") || "all";
const authorBookGrid = document.getElementById("book-grid");
const bookSearchInput = document.getElementById("book-search-input");
const bookSearchClear = document.getElementById("book-search-clear");
let currentSearchQuery = localStorage.getItem("dantalian_book_search") || "";

function loadCollapsedAuthorGroups() {
    try {
        const saved = JSON.parse(localStorage.getItem("dantalian_collapsed_author_groups") || "[]");
        return Array.isArray(saved) ? saved : [];
    } catch {
        return [];
    }
}

const collapsedAuthorGroups = new Set(loadCollapsedAuthorGroups());

function saveCollapsedAuthorGroups() {
    localStorage.setItem("dantalian_collapsed_author_groups", JSON.stringify([...collapsedAuthorGroups]));
}

function normalizeCd(cd) {
    return {
        id: cd.id,
        sourceType: "cd",
        originalId: cd.id,
        title: cd.title,
        cover_url: cd.cover_url,
        artist: cd.artist,
        publisher: cd.publisher,
        publish_date: cd.publish_date,
        authors: cd.authors || [],
        series_id: cd.series_id,
        copies_count: cd.copies_count || 0,
        lent_count: cd.lent_count || 0,
        media_type: cd.media_type || "cd",
        jan_code: cd.jan,
        disc_count: cd.disc_count,
        parent_book_id: cd.parent_book_id,
        label: cd.label,
        catalog_number: cd.catalog_number,
        tracks: cd.tracks || [],
        isbn: null,
        tanka_isbn: null,
        has_toc: false,
        primary_author: null,
        volume: null,
        series_number: null,
        jpno: null,
        ndl_url: null,
        toc_url: null,
        download_url: null,
    };
}

function normalizePlaylist(playlist) {
    return {
        id: playlist.id,
        sourceType: "playlist",
        originalId: playlist.id,
        title: playlist.name,
        cover_url: playlist.cover_url,
        description: playlist.description,
        tracks: playlist.tracks || [],
        media_type: "playlist",
        artist: "プレイリスト",
        authors: [],
        copies_count: 0,
        lent_count: 0,
        publish_date: null,
        series_id: null,
        volume: null,
        series_number: null,
    };
}

function getAllItems() {
    let items = allBooks.map((b) => Object.assign({ sourceType: "book", originalId: b.id }, b));
    if (allCds && allCds.length > 0) {
        items = items.concat(allCds.map(normalizeCd));
    }
    if (allLibraryPlaylists && allLibraryPlaylists.length > 0) {
        items = items.concat(allLibraryPlaylists.map(normalizePlaylist));
    }
    return items;
}

document.querySelectorAll(".view-tab").forEach((tab) => {
    tab.addEventListener("click", () => {
        document.querySelectorAll(".view-tab").forEach((t) => t.classList.remove("active"));
        tab.classList.add("active");
        currentView = tab.dataset.view;
        localStorage.setItem("dantalian_view", currentView);
        renderItems();
    });
});

document.getElementById("sort-buttons").addEventListener("click", (e) => {
    const btn = e.target.closest(".width-btn");
    if (!btn) return;
    currentSort = btn.dataset.sort;
    localStorage.setItem("dantalian_sort", currentSort);
    document.querySelectorAll("#sort-buttons .width-btn").forEach((b) => {
        b.classList.toggle("active", b.dataset.sort === currentSort);
    });
    renderItems();
});

bookSearchInput.value = currentSearchQuery;
bookSearchClear.classList.toggle("hidden", currentSearchQuery.length === 0);

bookSearchInput.addEventListener("input", () => {
    currentSearchQuery = bookSearchInput.value.trim();
    localStorage.setItem("dantalian_book_search", currentSearchQuery);
    bookSearchClear.classList.toggle("hidden", currentSearchQuery.length === 0);
    renderItems();
});

bookSearchClear.addEventListener("click", () => {
    bookSearchInput.value = "";
    currentSearchQuery = "";
    localStorage.removeItem("dantalian_book_search");
    bookSearchClear.classList.add("hidden");
    renderItems();
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

function getPrimaryCreator(item) {
    if (item.sourceType === "cd") {
        const artist = String(item.artist || getPrimaryAuthor(item)?.name || "").trim();
        return artist ? { key: `artist:${normalizeSearchText(artist)}`, name: artist } : null;
    }
    if (item.sourceType === "playlist") {
        return { key: "playlist", name: "プレイリスト" };
    }
    const author = getPrimaryAuthor(item);
    return author ? { key: `author:${author.id}`, name: author.name } : null;
}

function normalizeSearchText(value) {
    return String(value || "")
        .toLowerCase()
        .normalize("NFKC")
        .replace(/[ぁ-ゖ]/g, (ch) => String.fromCharCode(ch.charCodeAt(0) + 0x60))
        .replace(/\s+/g, "");
}

function getItemSearchText(item) {
    const series = item.series_id != null ? allSeries.find((s) => s.id === item.series_id) : null;
    const grandSeriesNames = [];
    for (const gs of allGrandSeries) {
        if (gs.items.some((it) =>
            (it.item_type === "book" && it.item_id === item.originalId) ||
            (it.item_type === "cd" && item.sourceType === "cd" && it.item_id === item.originalId) ||
            (it.item_type === "series" && ((item.sourceType === "book" && item.series_id === it.item_id) || (item.sourceType === "cd" && item.series_id === it.item_id)))
        )) {
            grandSeriesNames.push(gs.name);
        }
    }
    return [
        item.title,
        item.isbn,
        item.jan_code,
        item.publisher,
        item.artist,
        item.jpno,
        item.ndl_url,
        series?.name,
        ...grandSeriesNames,
        ...(item.authors || []).flatMap((a) => [a.name, a.transcription, a.ndl_id]),
    ].map(normalizeSearchText).join(" ");
}

function getFilteredItems() {
    const query = normalizeSearchText(currentSearchQuery);
    const allItems = getAllItems();
    let items = allItems;
    if (query) {
        items = items.filter((item) => getItemSearchText(item).includes(query));
    }
    if (currentTypeFilter !== "all") {
        if (currentTypeFilter === "cd") {
            items = items.filter((item) => item.sourceType === "cd" && item.media_type === "cd");
        } else if (currentTypeFilter === "audiobook") {
            items = items.filter((item) => item.sourceType === "cd" && item.media_type === "audiobook");
        } else if (currentTypeFilter === "playlist") {
            items = items.filter((item) => item.sourceType === "playlist");
        } else {
            items = items.filter((item) => item.sourceType === "book" && item.media_type !== "cd" && item.media_type !== "audiobook");
        }
    }
    return items;
}

function getGrandSeriesItemIds(gs) {
    const ids = { books: new Set(), cds: new Set() };
    const allItems = getAllItems();
    for (const item of gs.items) {
        if (item.item_type === "book") {
            ids.books.add(item.item_id);
        } else if (item.item_type === "cd") {
            ids.cds.add(item.item_id);
        } else if (item.item_type === "series") {
            for (const it of allItems) {
                if (it.series_id === item.item_id) {
                    if (it.sourceType === "book") ids.books.add(it.originalId);
                    else ids.cds.add(it.originalId);
                }
            }
        }
    }
    return ids;
}

function groupByUserSeries(items) {
    const groups = {};
    const singles = [];

    for (const item of items) {
        if (item.series_id != null) {
            if (!groups[item.series_id]) {
                const s = allSeries.find((x) => x.id === item.series_id);
                groups[item.series_id] = {
                    series_id: item.series_id,
                    series_name: s ? s.name : `シリーズ ${item.series_id}`,
                    books: [],
                };
            }
            groups[item.series_id].books.push(item);
        } else {
            singles.push(item);
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

function renderBookCard(item) {
    const copiesBadge = item.copies_count > 0
        ? ` <span class="copy-count-badge${item.lent_count > 0 ? ' copy-lent-badge' : ''}">${item.copies_count}${item.lent_count > 0 ? ' (' + item.lent_count + '貸出)' : ''}</span>`
        : "";
    const mediaType = item.media_type || "book";
    const mediaBadges = { cd: "CD", audiobook: "AB" };
    const mediaBadge = mediaBadges[mediaType] ? `<span class="media-badge media-badge--${mediaType}">${mediaBadges[mediaType]}</span>` : "";
    const clickHandler = item.sourceType === "cd" ? `showCdDetail(${item.originalId})` : `showDetail(${item.id})`;
    return `
    <div class="book-card" onclick="${clickHandler}">
        ${
            item.cover_url
                ? `<img class="book-cover" src="/images/${item.cover_url}" alt="${escapeAttr(item.title)}" loading="lazy">`
                : '<div class="book-cover-placeholder">No Image</div>'
        }
        ${mediaBadge}
        <div class="book-info">
            <div class="book-title">${escapeHtml(item.title)}${copiesBadge}</div>
            ${item.authors && item.authors.length > 0 ? `<div class="book-author">${item.authors.map((a) => escapeHtml(a.name)).join(", ")}</div>` : ""}
            ${item.publisher ? `<div class="book-meta">${escapeHtml(item.publisher)}</div>` : ""}
        </div>
    </div>`;
}

function renderCdCard(item) {
    const mediaType = item.media_type || "cd";
    const mediaBadges = { cd: "CD", audiobook: "AB" };
    const mediaBadge = mediaBadges[mediaType] ? `<span class="media-badge media-badge--${mediaType}">${mediaBadges[mediaType]}</span>` : "";

    let discCount = item.disc_count || 1;
    let tracks = item.tracks || [];
    if (tracks.length > 0) {
        const maxDisc = Math.max(...tracks.map((t) => t.disc_number || 1), 1);
        discCount = Math.max(discCount, maxDisc);
    }

    let tracksHtml = "";
    if (tracks.length > 0) {
        const MAX_DISPLAY = 6;
        const shown = tracks.slice(0, MAX_DISPLAY);
        const remaining = tracks.length - MAX_DISPLAY;

        const lines = shown.map((t) => {
            const hasAudio = t.file_hash ? ' cd-card-track-has-audio' : '';
            return `<span class="cd-card-track${hasAudio}">${String(t.track_number).padStart(2, "0")}. ${escapeHtml(t.title)}${t.duration ? ` <span class="cd-card-duration">${escapeHtml(t.duration)}</span>` : ""}</span>`;
        }).join("");

        if (lines) {
            tracksHtml = `<div class="cd-card-tracks">${lines}</div>`;
            if (remaining > 0) {
                tracksHtml += `<div class="cd-card-more">他 ${remaining} 曲</div>`;
            }
        }
    } else if (discCount > 1) {
        tracksHtml = `<div class="cd-card-tracks-empty">${discCount}枚組</div>`;
    }

    const subParts = [];
    if (item.artist) {
        subParts.push(escapeHtml(item.artist));
    } else if (item.authors && item.authors.length > 0) {
        subParts.push(item.authors.map((a) => escapeHtml(a.name)).join(", "));
    }
    if (item.label || item.catalog_number) {
        subParts.push([item.label, item.catalog_number].filter(Boolean).join(" · "));
    }

    return `
    <div class="book-card cd-card-v" onclick="showCdDetail(${item.originalId})">
        ${
            item.cover_url
                ? `<img class="book-cover" src="/images/${item.cover_url}" alt="${escapeAttr(item.title)}" loading="lazy">`
                : '<div class="book-cover-placeholder">No Image</div>'
        }
        ${mediaBadge}
        <div class="book-info">
            <div class="book-title">${escapeHtml(item.title)}</div>
            ${subParts.length > 0 ? `<div class="book-author">${subParts.join(" / ")}</div>` : ""}
            ${tracksHtml}
        </div>
    </div>`;
}

function renderPlaylistCard(item) {
    const trackCount = (item.tracks || []).filter((entry) => entry.track && entry.track.file_hash).length;
    return `
    <div class="book-card cd-card-v playlist-card" onclick="location.href='/music/?playlist=${item.originalId}'">
        ${
            item.cover_url
                ? `<img class="book-cover" src="/images/${item.cover_url}" alt="${escapeAttr(item.title)}" loading="lazy">`
                : '<div class="book-cover-placeholder"><span class="material-icons">queue_music</span></div>'
        }
        <span class="media-badge media-badge--playlist">PL</span>
        <div class="book-info">
            <div class="book-title">${escapeHtml(item.title)}</div>
            <div class="book-author">プレイリスト</div>
            <div class="cd-card-tracks-empty">${trackCount} 曲</div>
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
    const directCdCount = gs.items.filter((it) => it.item_type === "cd").length;
    const subInfo = [];
    if (seriesCount > 0) subInfo.push(`${seriesCount}シリーズ`);
    if (directBookCount > 0) subInfo.push(`${directBookCount}冊`);
    if (directCdCount > 0) subInfo.push(`${directCdCount}枚`);
    const subInfoStr = subInfo.join(" + ");

    return `
    <div class="grand-series-card" onclick="showGrandSeriesModal(${gs.id})">
        <div class="series-covers grand-series-covers">${coversHtml}</div>
        <div class="series-info">
            <div class="series-label grand-label">大シリーズ</div>
            <div class="series-title">${escapeHtml(gs.name)}</div>
            <div class="series-count">${books.length}件${subInfoStr ? ` (${subInfoStr})` : ""}</div>
        </div>
    </div>`;
}

function buildGridHtml(items) {
    const seriesIdsInGrand = new Set();
    const bookIdsInGrand = new Set();
    const cdIdsInGrand = new Set();
    for (const gs of allGrandSeries) {
        const ids = getGrandSeriesItemIds(gs);
        for (const bid of ids.books) bookIdsInGrand.add(bid);
        for (const cid of ids.cds) cdIdsInGrand.add(cid);
        for (const item of gs.items) {
            if (item.item_type === "series") seriesIdsInGrand.add(item.item_id);
        }
    }

    const gridItems = [];

    for (const gs of allGrandSeries) {
        const ids = getGrandSeriesItemIds(gs);
        const gsItems = items.filter((it) =>
            (it.sourceType === "book" && ids.books.has(it.originalId)) ||
            (it.sourceType === "cd" && ids.cds.has(it.originalId))
        );
        if (gsItems.length === 0) continue;
        gridItems.push({ type: "grand_series", gs, books: gsItems });
    }

    const { groups, singles } = groupByUserSeries(items);

    for (const [sid, series] of Object.entries(groups)) {
        if (seriesIdsInGrand.has(parseInt(sid, 10))) continue;
        gridItems.push({ type: "series", series, books: series.books });
    }

    for (const item of singles) {
        if (
            (item.sourceType === "book" && bookIdsInGrand.has(item.originalId)) ||
            (item.sourceType === "cd" && cdIdsInGrand.has(item.originalId))
        ) continue;
        gridItems.push({ type: "item", item, books: [item] });
    }

    for (const item of gridItems) {
        item._sorted = sortBookList(item.books);
    }

    function getItemVolumeKey(item) {
        if (item._sorted.length > 0) return parseVolumeSortKey(item._sorted[0].volume);
        return [99, 0];
    }

    gridItems.sort((a, b) => {
        if (currentSort === "title") {
            const nameA = a.type === "grand_series" ? a.gs.name : a.type === "series" ? a.series.series_name : a.item.title;
            const nameB = b.type === "grand_series" ? b.gs.name : b.type === "series" ? b.series.series_name : b.item.title;
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
    for (const item of gridItems) {
        const sortedBooks = item._sorted;
        if (item.type === "grand_series") {
            html += renderGrandSeriesCard(item.gs, sortedBooks);
        } else if (item.type === "series") {
            html += renderSeriesCard(item.series, sortedBooks);
        } else if (item.item.sourceType === "cd") {
            html += renderCdCard(item.item);
        } else if (item.item.sourceType === "playlist") {
            html += renderPlaylistCard(item.item);
        } else {
            html += renderBookCard(item.item);
        }
    }
    return html;
}

function renderItemsByAuthor(items) {
    const authorMap = new Map();

    for (const item of items) {
        const primary = getPrimaryCreator(item);
        if (!primary) {
            if (!authorMap.has("__none__")) authorMap.set("__none__", { author: null, books: [] });
            authorMap.get("__none__").books.push(item);
            continue;
        }
        if (!authorMap.has(primary.key)) {
            authorMap.set(primary.key, { author: primary, books: [] });
        }
        authorMap.get(primary.key).books.push(item);
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
                    <span class="author-group-count">${books.length}件</span>
                </button>`
            : `<button type="button" class="author-group-header author-group-header--none" aria-expanded="${!isCollapsed}">
                    <span class="author-group-name">アーティスト未設定</span>
                    <span class="author-group-count">${books.length}件</span>
                </button>`;

        html += `<div class="author-group${isCollapsed ? " author-group--collapsed" : ""}" data-author-group="${escapeHtml(groupKey)}">${headerHtml}<div class="author-group-grid">`;
        html += buildGridHtml(books);
        html += `</div></div>`;
    }

    return html;
}

function renderItems() {
    const allItems = getAllItems();
    if (allItems.length === 0) {
        bookGrid.innerHTML = '<p class="empty-state">ISBN/JANで書籍またはCDを登録してください</p>';
        bookCount.textContent = "(0件)";
        return;
    }

    const items = getFilteredItems();
    const totalCount = getAllItems().length;
    bookCount.textContent = currentSearchQuery
        ? `(${items.length} / ${totalCount}件)`
        : `(${totalCount}件)`;

    if (items.length === 0) {
        bookGrid.className = "";
        bookGrid.innerHTML = '<p class="empty-state">該当するアイテムがありません</p>';
        return;
    }

    if (currentView === "author") {
        bookGrid.className = "book-grid-author";
        bookGrid.innerHTML = renderItemsByAuthor(items);
    } else {
        bookGrid.className = "";
        bookGrid.id = "book-grid";
        bookGrid.innerHTML = buildGridHtml(items);
    }
}

(function initSortButtons() {
    document.querySelectorAll("#sort-buttons .width-btn").forEach((btn) => {
        btn.classList.toggle("active", btn.dataset.sort === currentSort);
    });
})();

(function initViewTabs() {
    document.querySelectorAll(".view-tab").forEach((tab) => {
        tab.classList.toggle("active", tab.dataset.view === currentView);
    });
})();

(function initTypeFilterButtons() {
    document.querySelectorAll("#type-filter-buttons .width-btn").forEach((btn) => {
        btn.classList.toggle("active", btn.dataset.type === currentTypeFilter);
    });
})();

document.getElementById("type-filter-buttons").addEventListener("click", (e) => {
    const btn = e.target.closest(".width-btn");
    if (!btn) return;
    currentTypeFilter = btn.dataset.type;
    localStorage.setItem("dantalian_type_filter", currentTypeFilter);
    document.querySelectorAll("#type-filter-buttons .width-btn").forEach((b) => {
        b.classList.toggle("active", b.dataset.type === currentTypeFilter);
    });
    renderItems();
});
