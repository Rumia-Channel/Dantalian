function bookReaderUrl(bookId, fileId) {
    const parameters = new URLSearchParams({
        book: String(bookId),
        from: location.pathname + location.search + location.hash,
    });
    if (fileId != null) parameters.set("file", String(fileId));
    return `/reader/?${parameters}`;
}

function isCatalogueBook(item) {
    return item.sourceType !== "cd" && item.media_type !== "cd" && item.media_type !== "audiobook";
}

// Web 小説はローカルに書籍レコードが無いので、専用の読書 URL へ遷移する。
function narouReaderUrl(narouId) {
    const parameters = new URLSearchParams({
        narou: String(narouId),
        from: location.pathname + location.search + location.hash,
    });
    return `/reader/?${parameters}`;
}

function catalogueItemClickHandler(item) {
    if (item.sourceType === "cd") return `showCdDetail(${item.originalId})`;
    if (item.sourceType === "narou") return `location.href='${escapeJs(narouReaderUrl(item.narou_id))}'`;
    if (document.body.dataset.catalogue !== "books" || !isCatalogueBook(item)) return `showDetail(${item.id})`;
    return `location.href='${escapeJs(bookReaderUrl(item.id))}'`;
}

function renderBookFileList(book, editable = false) {
    const prefix = editable ? "edit" : "detail";
    if (!book.files?.length) return `<div class="${prefix}-epub-empty">ファイル未登録</div>`;
    return book.files.map((file) => `<div class="${prefix}-epub-${editable ? "current" : "info"}">
        <span class="${prefix}-epub-name">${escapeHtml(file.file_name)}</span>
        <a class="btn btn-xs btn-outline-accent" href="${escapeAttr(bookReaderUrl(book.id, file.id))}">読む</a>
        <a class="btn btn-xs btn-ghost" href="/epubs/${encodeURIComponent(file.file_hash)}" download="${escapeAttr(file.file_name)}">ダウンロード</a>
        ${editable ? `<button type="button" class="btn btn-xs btn-outline-danger" onclick="deleteBookFile(${book.id}, ${file.id})">削除</button>` : ""}
    </div>`).join("");
}

function dlsiteWorkUrl(productId) {
    return `https://www.dlsite.com/home/work/=/product_id/${encodeURIComponent(productId)}.html`;
}
