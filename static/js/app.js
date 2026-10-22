const bookGrid = document.getElementById("book-grid");
const bookCount = document.getElementById("book-count");
const detailOverlay = document.getElementById("detail-overlay");
const detailContent = document.getElementById("detail-content");

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
            <button type="button" class="btn btn-ghost" onclick="initializeHome()">再読み込み</button>
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
    if (results.every(Boolean)) renderItems();
    else renderHomeLoadError();
}

initializeHome();
