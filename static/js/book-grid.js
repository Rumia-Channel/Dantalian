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

function renderBooks() {
    if (allBooks.length === 0) {
        bookGrid.innerHTML = '<p class="empty-state">ISBNで書籍を登録してください</p>';
        return;
    }

    const seriesIdsInGrand = new Set();
    const bookIdsInGrand = new Set();
    for (const gs of allGrandSeries) {
        for (const item of gs.items) {
            if (item.item_type === "series") seriesIdsInGrand.add(item.item_id);
            if (item.item_type === "book") bookIdsInGrand.add(item.item_id);
        }
    }

    const { groups, singles } = groupByUserSeries(allBooks);
    let html = "";

    for (const gs of allGrandSeries) {
        const gsBookIds = getGrandSeriesBookIds(gs);
        const gsBooks = allBooks.filter((b) => gsBookIds.has(b.id));
        if (gsBooks.length === 0) continue;

        gsBooks.sort((a, b) => a.id - b.id);
        const coversHtml = gsBooks.slice(0, 12).map((b) =>
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

        html += `
        <div class="grand-series-card" onclick="showGrandSeriesModal(${gs.id})">
            <div class="series-covers grand-series-covers">${coversHtml}</div>
            <div class="series-info">
                <div class="series-label grand-label">大シリーズ</div>
                <div class="series-title">${escapeHtml(gs.name)}</div>
                <div class="series-count">${gsBooks.length}冊${subInfoStr ? ` (${subInfoStr})` : ""}</div>
            </div>
        </div>`;
    }

    const seriesEntries = Object.entries(groups).sort((a, b) =>
        b[1].books[0].id - a[1].books[0].id
    );

    for (const [sid, series] of seriesEntries) {
        if (seriesIdsInGrand.has(parseInt(sid, 10))) continue;
        series.books.sort((a, b) => a.id - b.id);
        const coversHtml = series.books.slice(0, 8).map((b) =>
            b.cover_url
                ? `<img class="book-cover" src="/images/${b.cover_url}" alt="" loading="lazy">`
                : '<div class="book-cover-placeholder"></div>'
        ).join("");

        html += `
        <div class="series-card" onclick="showSeriesModal(${series.series_id})">
            <div class="series-covers">${coversHtml}</div>
            <div class="series-info">
                <div class="series-label">シリーズ</div>
                <div class="series-title">${escapeHtml(series.series_name)}</div>
                <div class="series-count">${series.books.length}冊</div>
            </div>
        </div>`;
    }

    for (const book of singles) {
        if (bookIdsInGrand.has(book.id)) continue;
        html += `
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

    bookGrid.innerHTML = html;
}
