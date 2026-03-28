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
