const bookGrid = document.getElementById("book-grid");
const bookCount = document.getElementById("book-count");
const detailOverlay = document.getElementById("detail-overlay");
const detailContent = document.getElementById("detail-content");

let allBooks = [];
let allSeries = [];
let allGrandSeries = [];

function escapeHtml(text) {
    if (text == null) return "";
    const div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
}

function escapeAttr(text) {
    return text.replace(/'/g, "\\'").replace(/"/g, "&quot;");
}

async function loadBooks() {
    try {
        const res = await fetch("/api/books");
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        allBooks = await res.json();
        bookCount.textContent = `(${allBooks.length}冊)`;
        renderBooks();
    } catch (err) {
        console.error("loadBooks failed:", err);
        bookGrid.innerHTML = '<p class="empty-state">読み込みに失敗しました</p>';
    }
}

async function loadSeries() {
    try {
        const res = await fetch("/api/series");
        allSeries = await res.json();
    } catch {
        allSeries = [];
    }
}

async function loadGrandSeries() {
    try {
        const res = await fetch("/api/grand-series");
        allGrandSeries = await res.json();
    } catch {
        allGrandSeries = [];
    }
}
