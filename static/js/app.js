const bookGrid = document.getElementById("book-grid");
const bookCount = document.getElementById("book-count");
const detailOverlay = document.getElementById("detail-overlay");
const detailContent = document.getElementById("detail-content");

(async () => {
    await loadSeries();
    await loadGrandSeries();
    await loadBooks();
    renderBooks();
})();
