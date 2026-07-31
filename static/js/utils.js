let allBooks = [];
let allCds = [];
let allSeries = [];
let allGrandSeries = [];
let allStorageLocations = [];
let allLabels = [];
let previewAudio = null;

function escapeHtml(text) {
    if (text == null) return "";
    const div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
}

function escapeAttr(text) {
    if (text == null) return "";
    // HTML 属性コンテキスト用。& " ' を実体参照化（バックスラッシュは使わない）。
    // 二重引用符で囲んだ属性 (value="...") でも、onclick="fn('...')" のような
    // JS 文字列コンテキストでも正しく復号される (&#39; / &quot; は HTML パーサが ' " に戻す)。
    return String(text)
        .replace(/&/g, "&amp;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#39;");
}

// インラインイベントハンドラ内の JS 文字列リテラル用 (onclick="fn('${escapeJs(x)}')")。
// \ と引用符・改行をバックスラッシュエスケープする。value="..." のような通常属性には
// 使わないこと (\\ がリテラルとして残る)。通常属性には escapeAttr を使う。
function escapeJs(text) {
    if (text == null) return "";
    return String(text)
        .replace(/\\/g, "\\\\")
        .replace(/'/g, "\\'")
        .replace(/"/g, '\\"')
        .replace(/\r/g, "\\r")
        .replace(/\n/g, "\\n")
        .replace(/\u2028/g, "\\u2028")
        .replace(/\u2029/g, "\\u2029");
}

function normalizePublishDateInput(value) {
    const raw = String(value ?? "").trim();
    if (!raw) return "";

    const ascii = raw
        .replace(/[０-９]/g, (ch) => String.fromCharCode(ch.charCodeAt(0) - 0xfee0))
        .replace(/[－／．]/g, (ch) => ({ "－": "-", "／": "/", "．": "." })[ch]);
    let parts;
    if (/^[0-9]+$/.test(ascii)) {
        if (ascii.length === 4) parts = [ascii.slice(0, 4)];
        else if (ascii.length === 6) parts = [ascii.slice(0, 4), ascii.slice(4, 6)];
        else if (ascii.length === 8) parts = [ascii.slice(0, 4), ascii.slice(4, 6), ascii.slice(6, 8)];
        else return "";
    } else {
        parts = ascii
            .replace(/年/g, "-")
            .replace(/月/g, "-")
            .replace(/日/g, "")
            .split(/[-/.]/)
            .filter(Boolean);
    }

    const year = Number(parts[0]);
    if (parts.length < 1 || !/^\d{4}$/.test(parts[0]) || year < 1900 || year > 2999) return "";
    if (parts.length === 1) return `${String(year).padStart(4, "0")}-NN-NN`;

    const monthText = String(parts[1]).toUpperCase();
    if (parts.length === 2) {
        if (monthText === "NN") return `${String(year).padStart(4, "0")}-NN-NN`;
        const month = Number(monthText);
        return month >= 1 && month <= 12
            ? `${String(year).padStart(4, "0")}-${String(month).padStart(2, "0")}-NN`
            : "";
    }
    if (parts.length !== 3) return "";

    const dayText = String(parts[2]).toUpperCase();
    if (monthText === "NN" && dayText === "NN") return `${String(year).padStart(4, "0")}-NN-NN`;
    const month = Number(monthText);
    if (month < 1 || month > 12) return "";
    if (dayText === "NN") return `${String(year).padStart(4, "0")}-${String(month).padStart(2, "0")}-NN`;

    const day = Number(dayText);
    const daysInMonth = new Date(Date.UTC(year, month, 0)).getUTCDate();
    return day >= 1 && day <= daysInMonth
        ? `${String(year).padStart(4, "0")}-${String(month).padStart(2, "0")}-${String(day).padStart(2, "0")}`
        : "";
}

function bindPublishDateInputs(root = document) {
    root.querySelectorAll("input[data-publish-date]").forEach((input) => {
        const normalize = () => {
            const raw = input.value.trim();
            if (!raw) {
                input.setCustomValidity("");
                return;
            }
            const normalized = normalizePublishDateInput(raw);
            if (normalized) {
                input.value = normalized;
                input.setCustomValidity("");
            } else {
                input.setCustomValidity("YYYY-MM-DD または YYYY-MM-NN 形式で入力してください");
            }
        };
        normalize();
        input.addEventListener("input", () => input.setCustomValidity(""));
        input.addEventListener("blur", normalize);
    });
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

async function loadStorageLocations() {
    try {
        const res = await fetch("/api/storage-locations");
        allStorageLocations = await res.json();
    } catch {
        allStorageLocations = [];
    }
}

async function loadLabels() {
    try {
        const res = await fetch("/api/labels");
        allLabels = await res.json();
    } catch {
        allLabels = [];
    }
}

function getStorageLocationPath(locationId) {
    if (locationId == null) return "";
    const parts = [];
    let current = allStorageLocations.find((l) => l.id === locationId);
    while (current) {
        parts.unshift(current.name);
        current = current.parent_id != null
            ? allStorageLocations.find((l) => l.id === current.parent_id)
            : null;
    }
    return parts.join(" > ");
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

function findCdGrandSeries(cdId) {
    for (const gs of allGrandSeries) {
        if (gs.items.some((it) => it.item_type === "cd" && it.item_id === cdId)) return gs;
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

function stopPreviewAudio() {
    if (!previewAudio) return;
    previewAudio.pause();
    previewAudio.removeAttribute("src");
    previewAudio.load();
    previewAudio.remove();
    previewAudio = null;
}

function playPreviewAudio(url, title) {
    stopPreviewAudio();
    const persistentPlayer = document.getElementById("dantalian-audio-player");
    if (persistentPlayer) {
        persistentPlayer.pause();
        persistentPlayer.remove();
    }

    const player = document.createElement("audio");
    player.preload = "auto";
    player.hidden = true;
    player.setAttribute("aria-hidden", "true");
    player.setAttribute("data-title", title || "");
    player.src = url;
    player.addEventListener("ended", () => {
        if (previewAudio === player) stopPreviewAudio();
    }, { once: true });
    player.addEventListener("error", () => {
        if (previewAudio === player) stopPreviewAudio();
    }, { once: true });
    document.body.appendChild(player);
    previewAudio = player;

    const playPromise = player.play();
    if (playPromise && typeof playPromise.catch === "function") {
        playPromise.catch(() => {
            if (previewAudio === player) stopPreviewAudio();
        });
    }
}

function playAudio(url, title) {
    stopPreviewAudio();
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
