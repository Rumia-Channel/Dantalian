let allBooks = [];
let allCds = [];
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

async function loadBooks() {
    try {
        const res = await fetch("/api/books");
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        allBooks = await res.json();
    } catch (err) {
        console.error("loadBooks failed:", err);
        allBooks = [];
    }
}

async function loadCds() {
    try {
        const res = await fetch("/api/cds");
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        allCds = await res.json();
    } catch (err) {
        console.error("loadCds failed:", err);
        allCds = [];
    }
}

function findBookGrandSeries(bookId) {
    for (const gs of allGrandSeries) {
        if (gs.items.some((it) => it.item_type === "book" && it.item_id === bookId)) return gs;
    }
    return null;
}

function findSeriesGrandSeries(seriesId) {
    for (const gs of allGrandSeries) {
        if (gs.items.some((it) => it.item_type === "series" && it.item_id === seriesId)) return gs;
    }
    return null;
}

function getBookIndirectGrandSeriesIds(bookId) {
    const book = allBooks.find((b) => b.id === bookId);
    if (!book || book.series_id == null) return new Set();
    const ids = new Set();
    for (const gs of allGrandSeries) {
        if (gs.items.some((it) => it.item_type === "series" && it.item_id === book.series_id)) {
            ids.add(gs.id);
        }
    }
    return ids;
}

function isBookInGrandSeriesViaSeries(bookId) {
    const book = allBooks.find((b) => b.id === bookId);
    if (!book || book.series_id == null) return false;
    return findSeriesGrandSeries(book.series_id) != null;
}

function showConfirm(opts) {
    if (typeof opts === "string") opts = { message: opts };

    const message = opts.message || "";
    const okLabel = opts.okLabel || "OK";
    const cancelLabel = opts.cancelLabel || "キャンセル";
    const okClass = opts.okClass || "btn btn-sm btn-outline-danger";

    let overlay = document.getElementById("confirm-overlay");
    if (!overlay) {
        overlay = document.createElement("div");
        overlay.id = "confirm-overlay";
        overlay.className = "confirm-overlay hidden";
        overlay.innerHTML = `
            <div class="confirm-box">
                <div class="confirm-message" id="confirm-message"></div>
                <div class="confirm-actions">
                    <button class="btn btn-sm btn-ghost" id="confirm-cancel"></button>
                    <button class="btn btn-sm btn-outline-danger" id="confirm-ok"></button>
                </div>
            </div>`;
        document.body.appendChild(overlay);
    }

    const msgEl = document.getElementById("confirm-message");
    const okBtn = document.getElementById("confirm-ok");
    const cancelBtn = document.getElementById("confirm-cancel");

    msgEl.textContent = message;
    okBtn.textContent = okLabel;
    cancelBtn.textContent = cancelLabel;
    okBtn.className = okClass;
    overlay.classList.remove("hidden");

    return new Promise((resolve) => {
        function cleanup() {
            overlay.classList.add("hidden");
            okBtn.removeEventListener("click", onOk);
            cancelBtn.removeEventListener("click", onCancel);
            overlay.removeEventListener("click", onBg);
            document.removeEventListener("keydown", onKey);
        }

        function onOk() { cleanup(); resolve(true); }
        function onCancel() { cleanup(); resolve(false); }
        function onBg(e) { if (e.target === overlay) { cleanup(); resolve(false); } }
        function onKey(e) { if (e.key === "Escape") { cleanup(); resolve(false); } }

        okBtn.addEventListener("click", onOk);
        cancelBtn.addEventListener("click", onCancel);
        overlay.addEventListener("click", onBg);
        document.addEventListener("keydown", onKey);
        okBtn.focus();
    });
}

function playAudio(url, title) {
    let player = document.getElementById("dantalian-audio-player");
    if (!player) {
        player = document.createElement("audio");
        player.id = "dantalian-audio-player";
        player.controls = true;
        player.style.cssText = "position:fixed;bottom:0;left:0;right:0;z-index:9999;width:100%;background:var(--color-bg-surface);border-top:1px solid var(--color-border-light);";
        document.body.appendChild(player);
    }
    player.src = url;
    player.setAttribute("data-title", title);
    player.play();
}
