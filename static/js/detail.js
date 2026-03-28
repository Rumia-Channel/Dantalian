function showGrandSeriesModal(gsId) {
    const gs = allGrandSeries.find((g) => g.id === gsId);
    if (!gs) return;

    let contentHtml = "";

    for (const item of gs.items) {
        if (item.item_type === "series") {
            const series = allSeries.find((s) => s.id === item.item_id);
            const books = allBooks.filter((b) => b.series_id === item.item_id).sort((a, b) => a.id - b.id);
            contentHtml += `
            <div class="gs-modal-section">
                <div class="gs-modal-section-title">${escapeHtml(series ? series.name : item.name)}</div>
                <div class="series-modal-grid">
                    ${books.map((b) => `
                        <div class="series-modal-item" onclick="showDetail(${b.id})">
                            ${b.cover_url
                                ? `<img class="book-cover" src="/images/${b.cover_url}" alt="" loading="lazy">`
                                : '<div class="book-cover-placeholder">No Image</div>'}
                            <div class="volume-title">${escapeHtml(b.title)}</div>
                        </div>`
                    ).join("")}
                </div>
            </div>`;
        } else if (item.item_type === "book") {
            const book = allBooks.find((b) => b.id === item.item_id);
            if (book) {
                contentHtml += `
                <div class="gs-modal-section">
                    <div class="gs-modal-section-title">個別書籍</div>
                    <div class="series-modal-grid">
                        <div class="series-modal-item" onclick="showDetail(${book.id})">
                            ${book.cover_url
                                ? `<img class="book-cover" src="/images/${book.cover_url}" alt="" loading="lazy">`
                                : '<div class="book-cover-placeholder">No Image</div>'}
                            <div class="volume-title">${escapeHtml(book.title)}</div>
                        </div>
                    </div>
                </div>`;
            }
        }
    }

    detailContent.innerHTML = `
        <div class="detail-title" style="margin-bottom:1rem;">${escapeHtml(gs.name)}</div>
        ${contentHtml || '<p style="color:#555;">アイテムがありません</p>'}
    `;

    detailOverlay.classList.remove("hidden");
}

function showSeriesModal(seriesId) {
    const series = allSeries.find((s) => s.id === seriesId);
    if (!series) return;
    const books = allBooks.filter((b) => b.series_id === seriesId).sort((a, b) => a.id - b.id);

    const volumesHtml = books.map((b) => `
        <div class="series-modal-item" onclick="showDetail(${b.id})">
            ${
                b.cover_url
                    ? `<img class="book-cover" src="/images/${b.cover_url}" alt="" loading="lazy">`
                    : '<div class="book-cover-placeholder">No Image</div>'
            }
            <div class="volume-title">${escapeHtml(b.title)}</div>
        </div>`
    ).join("");

    detailContent.innerHTML = `
        <div class="detail-title" style="margin-bottom:1rem;">${escapeHtml(series.name)}</div>
        <div class="series-modal-grid">${volumesHtml}</div>
    `;

    detailOverlay.classList.remove("hidden");
}

function showDetail(id) {
    const book = allBooks.find((b) => b.id === id);
    if (!book) return;

    const currentSeries = book.series_id != null
        ? allSeries.find((s) => s.id === book.series_id)
        : null;
    const currentGrandSeries = findBookGrandSeries(book.id);

    const metaParts = [];
    if (book.publisher) metaParts.push(`<div><span class="detail-meta-label">出版社</span>${escapeHtml(book.publisher)}</div>`);
    if (book.publish_date) metaParts.push(`<div><span class="detail-meta-label">出版日</span>${escapeHtml(book.publish_date)}</div>`);
    if (book.price) metaParts.push(`<div><span class="detail-meta-label">価格</span>${escapeHtml(book.price)}</div>`);
    if (book.extent) metaParts.push(`<div><span class="detail-meta-label">ページ数</span>${escapeHtml(book.extent)}</div>`);
    if (book.isbn) metaParts.push(`<div><span class="detail-meta-label">ISBN</span>${escapeHtml(book.isbn)}</div>`);

    const seriesOptions = allSeries
        .map((s) => `<option value="${s.id}" ${s.id === book.series_id ? "selected" : ""}>${escapeHtml(s.name)}</option>`)
        .join("");

    const grandSeriesOptions = allGrandSeries
        .map((gs) => `<option value="${gs.id}" ${currentGrandSeries && gs.id === currentGrandSeries.id ? "selected" : ""}>${escapeHtml(gs.name)}</option>`)
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
                ${currentGrandSeries ? `<div class="detail-grand-series-name">${escapeHtml(currentGrandSeries.name)}</div>` : ""}
                ${currentSeries ? `<div class="detail-series-name">${escapeHtml(currentSeries.name)}</div>` : ""}
                <div class="detail-title">${escapeHtml(book.title)}</div>
                ${book.volume ? `<div class="detail-volume">${escapeHtml(book.volume)}</div>` : ""}
                ${book.alternative ? `<div class="detail-alternative">${escapeHtml(book.alternative)}</div>` : ""}
                ${book.authors && book.authors.length > 0 ? `<div class="detail-author">${book.authors.map((a) => escapeHtml(a.name)).join(", ")}</div>` : ""}
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
        <div class="detail-series-assign">
            <label>大シリーズ</label>
            <select onchange="assignGrandSeries(${book.id}, this.value)">
                <option value="">なし</option>
                ${grandSeriesOptions}
            </select>
        </div>
        <div class="detail-actions">
            <button class="book-delete" onclick="deleteBook(${book.id})">削除</button>
        </div>
    `;

    detailOverlay.classList.remove("hidden");
}

function closeDetail(e) {
    if (e && e.target !== detailOverlay) return;
    detailOverlay.classList.add("hidden");
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

async function assignGrandSeries(bookId, value) {
    const gsId = value === "" ? null : parseInt(value, 10);

    const currentGs = findBookGrandSeries(bookId);
    if (currentGs) {
        try {
            await fetch(`/api/grand-series/${currentGs.id}/items/book/${bookId}`, { method: "DELETE" });
        } catch {}
    }

    if (gsId != null) {
        try {
            await fetch(`/api/grand-series/${gsId}/items`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ item_type: "book", item_id: bookId }),
            });
        } catch {}
    }

    await loadGrandSeries();
    await loadBooks();
    showDetail(bookId);
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

document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") detailOverlay.classList.add("hidden");
});
