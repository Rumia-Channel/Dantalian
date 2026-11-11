const bookGrid = document.getElementById("book-grid");
const bookCount = document.getElementById("book-count");
const detailOverlay = document.getElementById("detail-overlay");
const detailContent = document.getElementById("detail-content");
let homeCatalogueReady = false;

// Web 小説 (narou.rs) のカタログが届いたら一覧だけを描き直す。
// ローカル書籍の読み込みは待たないため、失敗しても本地の本は消えない。
document.addEventListener("dantalian:narou-catalogue", () => {
    if (homeCatalogueReady) renderItems();
});

function renderHomeLoadError() {
    const errors = getDataLoadErrors();
    const details = Object.entries(errors)
        .map(([key, message]) => `${key}: ${message}`)
        .join(" / ");
    bookCount.textContent = "";
    bookGrid.innerHTML = `
        <div class="load-error" role="alert">
            <span class="material-icons" aria-hidden="true">cloud_off</span>
            <p>一覧データを読み込めませんでした。登録内容は削除されていません。</p>
            <small>${escapeHtml(details)}</small>
            <button type="button" class="btn btn-ghost" onclick="refreshHome()">再読み込み</button>
        </div>`;
}

async function initializeHome() {
    // The books-only catalogue does not render CDs or CD labels, so loading
    // them only delays the first meaningful render (CDs was the slowest call).
    const booksOnly = document.body.dataset.catalogue === "books";
    const results = await Promise.all([
        loadSeries(),
        loadGrandSeries(),
        loadStorageLocations(),
        ...(booksOnly ? [] : [loadLabels(), loadCds()]),
        loadBooks(),
    ]);
    homeCatalogueReady = results.every(Boolean);
    if (homeCatalogueReady) renderItems();
    else renderHomeLoadError();
    // Web 小説はブラウザが narou.rs から直接読む。ローカル書籍の表示を
    // 止めないため、別で走らせて届いた時点でもう一度描画する。
    if (typeof refreshNarouCatalogue === "function") void refreshNarouCatalogue();
    return homeCatalogueReady;
}

const refreshHome = observeDataChanges(initializeHome);
refreshHome();
