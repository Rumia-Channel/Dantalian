function bookReaderUrl(bookId) {
    const parameters = new URLSearchParams({
        book: String(bookId),
        from: location.pathname + location.search + location.hash,
    });
    return `/reader/?${parameters}`;
}

function isCatalogueBook(item) {
    return item.sourceType !== "cd" && item.media_type !== "cd" && item.media_type !== "audiobook";
}

function catalogueItemClickHandler(item) {
    if (item.sourceType === "cd") return `showCdDetail(${item.originalId})`;
    if (document.body.dataset.catalogue !== "books" || !isCatalogueBook(item)) return `showDetail(${item.id})`;
    return `location.href='${escapeJs(bookReaderUrl(item.id))}'`;
}
