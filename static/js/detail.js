// href 用 URL サニタイザ: http/https/相対パスのみ許可し javascript: 等を遮断
function safeUrl(url) {
    const s = String(url == null ? "" : url).trim();
    if (/^(https?:)?\/\//i.test(s) || s.startsWith("/") || s.startsWith("#")) return s;
    return "";
}

function showGrandSeriesModal(gsId) {
    const gs = allGrandSeries.find((g) => g.id === gsId);
    if (!gs) return;

    let contentHtml = "";
    const individualBooks = [];
    const seriesSections = [];

    for (const item of gs.items) {
        if (item.item_type === "series") {
            const series = allSeries.find((s) => s.id === item.item_id);
            const sBooks = allBooks.filter((b) => b.series_id === item.item_id);
            const sCds = (allCds || []).filter((c) => c.series_id === item.item_id);
            const sItems = [...sBooks.map((b) => Object.assign({ sourceType: "book", originalId: b.id }, b)),
                ...(sCds.map((c) => ({ sourceType: "cd", originalId: c.id, id: c.id, title: c.title, cover_url: c.cover_url })))];
            const sorted = typeof sortBookList === "function" ? sortBookList(sItems) : sItems;
            seriesSections.push({ series, sortedBooks: sorted });
        } else if (item.item_type === "book") {
            const book = allBooks.find((b) => b.id === item.item_id);
            if (book) individualBooks.push(Object.assign({ sourceType: "book", originalId: book.id }, book));
        } else if (item.item_type === "cd") {
            const cd = (allCds || []).find((c) => c.id === item.item_id);
            if (cd) individualBooks.push({ sourceType: "cd", originalId: cd.id, id: cd.id, title: cd.title, cover_url: cd.cover_url });
        }
    }

    for (const { series, sortedBooks } of seriesSections) {
        contentHtml += `
        <div class="gs-modal-section">
            <div class="gs-modal-section-title">${escapeHtml(series ? series.name : "")}</div>
            <div class="series-modal-grid">
                ${sortedBooks.map((b) => `
                    <div class="series-modal-item" onclick="${b.sourceType === 'cd' ? `showCdDetail(${b.originalId})` : `showDetail(${b.id})`}">
                        ${b.cover_url
                            ? `<img class="book-cover" src="/images/${b.cover_url}" alt="" loading="lazy">`
                            : '<div class="book-cover-placeholder">No Image</div>'}
                        <div class="volume-title">${escapeHtml(b.title)}</div>
                    </div>`
                ).join("")}
            </div>
        </div>`;
    }

    if (individualBooks.length > 0) {
        const sortedIndividuals = typeof sortBookList === "function" ? sortBookList(individualBooks) : individualBooks;
        contentHtml += `
        <div class="gs-modal-section">
            <div class="gs-modal-section-title">個別アイテム</div>
            <div class="series-modal-grid">
                ${sortedIndividuals.map((b) => `
                    <div class="series-modal-item" onclick="${b.sourceType === 'cd' ? `showCdDetail(${b.originalId})` : `showDetail(${b.id})`}">
                        ${b.cover_url
                            ? `<img class="book-cover" src="/images/${b.cover_url}" alt="" loading="lazy">`
                            : '<div class="book-cover-placeholder">No Image</div>'}
                        <div class="volume-title">${escapeHtml(b.title)}</div>
                    </div>`
                ).join("")}
            </div>
        </div>`;
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
    const sBooks = allBooks.filter((b) => b.series_id === seriesId);
    const sCds = (allCds || []).filter((c) => c.series_id === seriesId);
    const sItems = [
        ...sBooks.map((b) => Object.assign({ sourceType: "book", originalId: b.id }, b)),
        ...sCds.map((c) => ({ sourceType: "cd", originalId: c.id, id: c.id, title: c.title, cover_url: c.cover_url })),
    ];
    const sortedItems = typeof sortBookList === "function" ? sortBookList(sItems) : sItems;

    const volumesHtml = sortedItems.map((b) => `
        <div class="series-modal-item" onclick="${b.sourceType === 'cd' ? `showCdDetail(${b.originalId})` : `showDetail(${b.id})`}">
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

    fetch(`/api/books/${book.id}/copies`)
        .then((r) => r.json())
        .then((copies) => {
            if (book.media_type === "cd" || book.media_type === "audiobook") {
                fetch(`/api/books/${book.id}/tracks`)
                    .then((r) => r.json())
                    .then((tracks) => renderDetail(book, copies, currentSeries, currentGrandSeries, tracks))
                    .catch(() => renderDetail(book, copies, currentSeries, currentGrandSeries, []));
            } else {
                renderDetail(book, copies, currentSeries, currentGrandSeries, []);
            }
        })
        .catch(() => renderDetail(book, [], currentSeries, currentGrandSeries, []));
}

function renderDetail(book, copies, currentSeries, currentGrandSeries, tracks) {
    tracks = tracks || [];
    const metaParts = [];
    if (book.publisher) metaParts.push(`<div><span class="detail-meta-label">出版社</span>${escapeHtml(book.publisher)}</div>`);
    if (book.publish_date) metaParts.push(`<div><span class="detail-meta-label">出版日</span>${escapeHtml(book.publish_date)}</div>`);
    if (book.price) metaParts.push(`<div><span class="detail-meta-label">価格</span>${escapeHtml(book.price)}</div>`);
    if (book.extent) metaParts.push(`<div><span class="detail-meta-label">ページ数</span>${escapeHtml(book.extent)}</div>`);
    if (book.isbn) metaParts.push(`<div><span class="detail-meta-label">ISBN</span>${escapeHtml(book.isbn)}</div>`);
    if (book.isdn) metaParts.push(`<div><span class="detail-meta-label">ISDN</span>${escapeHtml(book.isdn)}</div>`);
    if (book.jan) metaParts.push(`<div><span class="detail-meta-label">JAN</span>${escapeHtml(book.jan)}</div>`);
    if (book.artist) metaParts.push(`<div><span class="detail-meta-label">アーティスト</span>${escapeHtml(book.artist)}</div>`);
    if (book.label) metaParts.push(`<div><span class="detail-meta-label">レーベル</span>${escapeHtml(book.label)}</div>`);
    if (book.catalog_number) metaParts.push(`<div><span class="detail-meta-label">品番</span>${escapeHtml(book.catalog_number)}</div>`);
    if (book.disc_count) metaParts.push(`<div><span class="detail-meta-label">ディスク</span>${book.disc_count}枚</div>`);

    const readingStatusLabels = { unread: "未読", reading: "読書中", completed: "読了" };
    const rsLabel = readingStatusLabels[book.reading_status] || book.reading_status || "未読";
    metaParts.push(`<div><span class="detail-meta-label">読書状況</span><span class="detail-reading-status rs-${book.reading_status || 'unread'}">${escapeHtml(rsLabel)}</span></div>`);

    if (book.storage_location_id != null) {
        const locPath = getStorageLocationPath(book.storage_location_id);
        if (locPath) metaParts.push(`<div><span class="detail-meta-label">保管場所</span>${escapeHtml(locPath)}</div>`);
    }

    if (book.label_id != null) {
        const lbl = allLabels.find((l) => l.id === book.label_id);
        if (lbl) metaParts.push(`<div><span class="detail-meta-label">レーベル</span>${escapeHtml(lbl.name)}</div>`);
    }

    const authorLinks = book.authors && book.authors.length > 0
        ? book.authors.map((a) => `<span class="detail-author-link" onclick="location.href='/authors/?edit=${a.id}'">${escapeHtml(a.name)}</span>`).join(", ")
        : "";

    let copiesHtml = "";
    if (copies.length > 0) {
        copiesHtml = `
        <div class="detail-copies">
            <div class="detail-copies-title">所蔵 (${copies.length}件)</div>
            ${copies.map((c) => `
                <div class="detail-copy-item${c.lent_to ? ' copy-lent' : ''}">
                    <span class="copy-type-icon">${c.copy_type === 'ebook' ? 'smartphone' : 'menu_book'}</span>
                    <span class="copy-location">${c.location ? escapeHtml(c.location) : '<span class="copy-no-location">未設定</span>'}</span>
                    ${c.lent_to
                        ? `<span class="copy-lent-badge">貸出中: ${escapeHtml(c.lent_to)}</span>`
                        : '<span class="copy-available-badge">所持</span>'}
                    ${c.due_date ? `<span class="copy-due-date">返却予定: ${escapeHtml(c.due_date)}</span>` : ''}
                </div>
            `).join("")}
        </div>`;
    } else {
        copiesHtml = `<div class="detail-copies detail-copies-empty">所蔵情報なし</div>`;
    }

    let tracksHtml = "";
    if (tracks.length > 0) {
        const hasAudio = tracks.some((t) => t.file_hash);
        const discGroups = {};
        for (const t of tracks) {
            const d = t.disc_number || 1;
            if (!discGroups[d]) discGroups[d] = [];
            discGroups[d].push(t);
        }
        const discKeys = Object.keys(discGroups).sort((a, b) => a - b);
        for (const d of discKeys) {
            const discTracks = discGroups[d].slice().sort((a, b) => a.track_number - b.track_number);
            const numLabel = (n) => discKeys.length > 1
                ? `${d}-${String(n).padStart(2, "0")}`
                : String(n).padStart(2, "0");
            const discLabel = discKeys.length > 1
                ? `<div class="detail-tracks-disc">Disc ${d} <span class="detail-tracks-disc-count">(${discTracks.length} トラック)</span></div>`
                : "";
            tracksHtml += `<div class="detail-tracks">${discLabel}
                <div class="detail-tracks-list">
                    ${discTracks.map((t) => `
                        <div class="detail-track-item${hasAudio && t.file_hash ? ' detail-track-has-audio' : ''}">
                            <span class="detail-track-num" title="Disc ${d} / Track ${t.track_number}">${numLabel(t.track_number)}</span>
                            <span class="detail-track-title">${escapeHtml(t.title)}</span>
                            ${t.duration ? `<span class="detail-track-duration">${escapeHtml(t.duration)}</span>` : ""}
                            ${t.file_hash ? ` <button class="btn btn-xs btn-ghost detail-track-play" onclick="event.stopPropagation();playAudio('/audio/${t.file_hash}','${escapeJs(t.title)}')" aria-label="再生">
                                <span class="material-icons" aria-hidden="true">play_arrow</span>
                            </button>` : ""}
                        </div>
                    `).join("")}
                </div>
            </div>`;
        }
    }

    detailContent.innerHTML = `
        <div class="detail-header">
            <div class="detail-cover">
                ${
                    book.cover_url
                        ? `<img class="book-cover" src="/images/${book.cover_url}" alt="${escapeAttr(book.title)}">`
                        : '<div class="book-cover-placeholder">No Image</div>'
                }
            </div>
            <div class="detail-title-block">
                ${currentGrandSeries ? `<div class="detail-grand-series-name">${escapeHtml(currentGrandSeries.name)}</div>` : ""}
                ${currentSeries ? `<div class="detail-series-name">${escapeHtml(currentSeries.name)}</div>` : ""}
                <div class="detail-title">${escapeHtml(book.title)}</div>
                ${book.volume ? `<div class="detail-volume">${escapeHtml(book.volume)}</div>` : ""}
                ${book.alternative ? `<div class="detail-alternative">${escapeHtml(book.alternative)}</div>` : ""}
                ${authorLinks ? `<div class="detail-author">${authorLinks}</div>` : ""}
                <div class="detail-meta-list">${metaParts.join("")}</div>
            </div>
        </div>
        ${copiesHtml}
        ${tracksHtml}
        ${renderChildrenInDetail(book.id)}
        <div class="detail-epub">
            <div class="detail-epub-title">ファイル</div>
            ${
                book.epub_file_hash
                    ? `<div class="detail-epub-info">
                        <span class="detail-epub-name">${escapeHtml(book.epub_file_name || book.epub_file_hash)}</span>
                        <a class="btn btn-xs btn-outline-success detail-epub-open" href="/epubs/${encodeURIComponent(book.epub_file_hash)}" target="_blank" rel="noopener">開く</a>
                        <a class="btn btn-xs btn-ghost detail-epub-download" href="/epubs/${encodeURIComponent(book.epub_file_hash)}" download="${escapeAttr(book.epub_file_name || book.epub_file_hash)}">ダウンロード</a>
                    </div>`
                    : `<div class="detail-epub-empty">ファイル未登録</div>`
            }
        </div>
        ${book.description ? `<div class="detail-description">${escapeHtml(book.description)}</div>` : ""}
        ${
            book.ndl_url
                ? `<a href="${escapeAttr(safeUrl(book.ndl_url))}" target="_blank" rel="noopener" class="detail-ndl-link">国立国会図書館で見る</a>`
                : ""
        }
        <div class="detail-series-assign">
            <label>シリーズ</label>
            <div id="detail-series-select-container"></div>
        </div>
        <div class="detail-series-assign">
            <label>巻</label>
            <input type="number" id="detail-series-number" class="form-input" style="max-width:6rem;" min="1" value="${book.series_number != null ? book.series_number : ''}" placeholder="—" onchange="updateSeriesNumber(${book.id}, this.value)">
        </div>
        <div class="detail-series-assign">
            <label>大シリーズ</label>
            <div id="detail-grand-series-select-container"></div>
        </div>
        <div class="detail-actions">
            <a href="/edit/?mode=book&book=${book.id}" class="btn btn-sm btn-outline-success">編集</a>
            <button class="btn btn-sm btn-outline-danger" onclick="deleteBook(${book.id})">削除</button>
        </div>
    `;

    createSearchableSelect(document.getElementById("detail-series-select-container"), {
        options: allSeries.map((s) => ({ value: s.id, label: s.name })),
        value: book.series_id,
        placeholder: "なし",
        onChange: (val) => assignSeries(book.id, val),
    });

    const indirectGsIds = getBookIndirectGrandSeriesIds(book.id);

    createSearchableSelect(document.getElementById("detail-grand-series-select-container"), {
        options: allGrandSeries.filter((gs) => !indirectGsIds.has(gs.id)).map((gs) => ({ value: gs.id, label: gs.name })),
        value: currentGrandSeries && !indirectGsIds.has(currentGrandSeries.id) ? currentGrandSeries.id : null,
        placeholder: indirectGsIds.size > 0 ? "シリーズ経由で所属中" : "なし",
        onChange: (val) => assignGrandSeries(book.id, val),
    });

    detailOverlay.classList.remove("hidden");
}

function closeDetail(e) {
    if (e && e.target !== detailOverlay) return;
    detailOverlay.classList.add("hidden");
}

async function assignSeries(bookId, value) {
    const seriesId = value != null ? value : null;
    const numInput = document.getElementById("detail-series-number");
    const seriesNumber = numInput && numInput.value ? parseInt(numInput.value, 10) : null;
    try {
        await fetch(`/api/books/${bookId}/series`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ series_id: seriesId, series_number: seriesNumber }),
        });
        await loadSeries();
        await loadBooks();
        renderItems();
    } catch {}
}

async function updateSeriesNumber(bookId, value) {
    const seriesNumber = value ? parseInt(value, 10) : null;
    try {
        await fetch(`/api/books/${bookId}/series`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ series_number: seriesNumber }),
        });
        await loadBooks();
        renderItems();
    } catch {}
}

async function assignGrandSeries(bookId, value) {
    const gsId = value != null ? value : null;

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
    await loadSeries();
    await loadBooks();
    renderItems();
    showDetail(bookId);
}

async function deleteBook(id) {
    const ok = await showConfirm({ message: "この書籍を削除しますか？", okLabel: "削除" });
    if (!ok) return;
    try {
        const res = await fetch(`/api/books/${id}`, { method: "DELETE" });
        if (res.ok) {
            detailOverlay.classList.add("hidden");
            await loadSeries();
            await loadGrandSeries();
            await loadBooks();
            renderItems();
        }
    } catch {}
}

function renderChildrenInDetail(bookId) {
    if (!allCds || allCds.length === 0) return "";
    const children = allCds.filter((cd) => cd.parent_book_id === bookId);
    if (children.length === 0) return "";

    return `<div class="detail-children">
        <div class="detail-children-title">関連CD/オーディオブック (${children.length}件)</div>
        ${children.map((cd) => `
            <div class="detail-child-item">
                <span class="media-badge media-badge--${cd.media_type === 'audiobook' ? 'audiobook' : 'cd'}" style="position:static;display:inline-block;vertical-align:middle;margin-right:0.5rem">
                    ${cd.media_type === 'audiobook' ? 'AB' : 'CD'}
                </span>
                <span>${escapeHtml(cd.title)}</span>
                ${cd.artist ? `<span style="color:var(--color-text-dim);margin-left:0.5rem">${escapeHtml(cd.artist)}</span>` : ""}
            </div>
        `).join("")}
    </div>`;
}

function showCdDetail(cdId) {
    const rawCd = (allCds || []).find((c) => c.id === cdId);
    if (!rawCd) return;
    const cd = Object.assign({ sourceType: "cd", originalId: cdId }, rawCd);
    const currentSeries = cd.series_id != null ? allSeries.find((s) => s.id === cd.series_id) : null;

    fetch(`/api/cds/${cdId}/tracks`)
        .then((r) => r.json())
        .then((tracks) => renderCdDetail(cd, currentSeries, tracks))
        .catch(() => renderCdDetail(cd, currentSeries, []));
}

function renderCdDetail(cd, currentSeries, tracks) {
    tracks = tracks || [];
    const mediaBadge = `<span class="media-badge media-badge--${cd.media_type === "audiobook" ? "audiobook" : "cd"}" style="position:static;display:inline-block;margin-bottom:0.5rem">${cd.media_type === "audiobook" ? "AB" : "CD"}</span>`;
    const metaParts = [];
    if (cd.publisher) metaParts.push(`<div><span class="detail-meta-label">出版社</span>${escapeHtml(cd.publisher)}</div>`);
    if (cd.publish_date) metaParts.push(`<div><span class="detail-meta-label">出版日</span>${escapeHtml(cd.publish_date)}</div>`);
    if (cd.jan_code || cd.jan) metaParts.push(`<div><span class="detail-meta-label">JAN</span>${escapeHtml(cd.jan_code || cd.jan)}</div>`);
    if (cd.disc_count) metaParts.push(`<div><span class="detail-meta-label">ディスク</span>${cd.disc_count}枚</div>`);
    if (cd.label) metaParts.push(`<div><span class="detail-meta-label">レーベル</span>${escapeHtml(cd.label)}</div>`);
    if (cd.catalog_number) metaParts.push(`<div><span class="detail-meta-label">品番</span>${escapeHtml(cd.catalog_number)}</div>`);

    const authorLinks = cd.authors && cd.authors.length > 0
        ? cd.authors.map((a) => `<span class="detail-author-link" onclick="location.href='/authors/?edit=${a.id}'">${escapeHtml(a.name)}</span>`).join(", ")
        : (cd.artist ? escapeHtml(cd.artist) : "");

    let tracksHtml = "";
    if (tracks.length > 0) {
        const hasAudio = tracks.some((t) => t.file_hash);
        const discGroups = {};
        for (const t of tracks) {
            const d = t.disc_number || 1;
            if (!discGroups[d]) discGroups[d] = [];
            discGroups[d].push(t);
        }
        const discKeys = Object.keys(discGroups).sort((a, b) => a - b);
        for (const d of discKeys) {
            const discTracks = discGroups[d].slice().sort((a, b) => a.track_number - b.track_number);
            const numLabel = (n) => discKeys.length > 1
                ? `${d}-${String(n).padStart(2, "0")}`
                : String(n).padStart(2, "0");
            const discLabel = discKeys.length > 1
                ? `<div class="detail-tracks-disc">Disc ${d} <span class="detail-tracks-disc-count">(${discTracks.length} トラック)</span></div>`
                : "";
            tracksHtml += `<div class="detail-tracks">${discLabel}
                <div class="detail-tracks-list">
                    ${discTracks.map((t) => `
                        <div class="detail-track-item${hasAudio && t.file_hash ? ' detail-track-has-audio' : ''}">
                            <span class="detail-track-num" title="Disc ${d} / Track ${t.track_number}">${numLabel(t.track_number)}</span>
                            <span class="detail-track-title">${escapeHtml(t.title)}</span>
                            ${t.duration ? `<span class="detail-track-duration">${escapeHtml(t.duration)}</span>` : ""}
                            ${t.file_hash ? ` <button class="btn btn-xs btn-ghost detail-track-play" onclick="event.stopPropagation();playAudio('/audio/${t.file_hash}','${escapeJs(t.title)}')" aria-label="再生">
                                <span class="material-icons" aria-hidden="true">play_arrow</span>
                            </button>` : ""}
                        </div>
                    `).join("")}
                </div>
            </div>`;
        }
    }

    detailContent.innerHTML = `
        <div class="detail-header">
            <div class="detail-cover">
                ${cd.cover_url
                    ? `<img class="book-cover" src="/images/${cd.cover_url}" alt="${escapeAttr(cd.title)}">`
                    : '<div class="book-cover-placeholder">No Image</div>'}
            </div>
            <div class="detail-title-block">
                ${mediaBadge}
                ${currentSeries ? `<div class="detail-series-name">${escapeHtml(currentSeries.name)}</div>` : ""}
                <div class="detail-title">${escapeHtml(cd.title)}</div>
                ${authorLinks ? `<div class="detail-author">${authorLinks}</div>` : ""}
                <div class="detail-meta-list">${metaParts.join("")}</div>
            </div>
        </div>
        ${tracksHtml}
        <div class="detail-series-assign">
            <label>シリーズ</label>
            <div id="cd-detail-series-select-container"></div>
        </div>
        <div class="detail-series-assign">
            <label>大シリーズ</label>
            <div id="cd-detail-grand-series-select-container"></div>
        </div>
        <div class="detail-actions">
            <button class="btn btn-sm btn-primary" onclick="openCdPlayer(${cd.originalId})">
                <span class="material-icons" style="font-size:1.1em;vertical-align:-0.15em;">play_arrow</span> 再生
            </button>
            <a href="/edit/?mode=cd&cd=${cd.originalId}" class="btn btn-sm btn-outline-success">編集</a>
            <button class="btn btn-sm btn-outline-danger" onclick="deleteCd(${cd.originalId})">削除</button>
        </div>
    `;

    createSearchableSelect(document.getElementById("cd-detail-series-select-container"), {
        options: allSeries.map((s) => ({ value: s.id, label: s.name })),
        value: cd.series_id,
        placeholder: "なし",
        onChange: (val) => assignCdSeries(cd.originalId, val),
    });

    const cdGs = findCdGrandSeries(cd.originalId);
    createSearchableSelect(document.getElementById("cd-detail-grand-series-select-container"), {
        options: allGrandSeries.map((gs) => ({ value: gs.id, label: gs.name })),
        value: cdGs ? cdGs.id : null,
        placeholder: "なし",
        onChange: (val) => assignCdGrandSeries(cd.originalId, val),
    });

    detailOverlay.classList.remove("hidden");
}

async function assignCdSeries(cdId, value) {
    const cd = (allCds || []).find((c) => c.id === cdId);
    const seriesId = value != null ? value : null;
    try {
        await fetch(`/api/cds/${cdId}`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ title: cd?.title || "", series_id: seriesId }),
        });
        await loadCds();
        renderItems();
    } catch {}
}

async function assignCdGrandSeries(cdId, value) {
    const gsId = value != null ? value : null;

    const currentGs = findCdGrandSeries(cdId);
    if (currentGs) {
        try {
            await fetch(`/api/grand-series/${currentGs.id}/items/cd/${cdId}`, { method: "DELETE" });
        } catch {}
    }

    if (gsId != null) {
        try {
            await fetch(`/api/grand-series/${gsId}/items`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ item_type: "cd", item_id: cdId }),
            });
        } catch {}
    }

    await loadGrandSeries();
    await loadCds();
    renderItems();
}

function openCdPlayer(cdId) {
    location.href = `/music/?play=${cdId}`;
}

async function deleteCd(id) {
    const ok = await showConfirm({ message: "このCDを削除しますか？", okLabel: "削除" });
    if (!ok) return;
    try {
        const res = await fetch(`/api/cds/${id}`, { method: "DELETE" });
        if (res.ok) {
            detailOverlay.classList.add("hidden");
            await loadCds();
            renderItems();
        }
    } catch {}
}

document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") detailOverlay.classList.add("hidden");
});
