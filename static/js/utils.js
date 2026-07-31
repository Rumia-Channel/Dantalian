let allBooks = [];
let allCds = [];
let allSeries = [];
let allGrandSeries = [];
let allStorageLocations = [];
let allLabels = [];
let previewAudio = null;
const dataLoadErrors = {};

// CD/オーディオブックを一覧・音楽ページ・プレイヤーで同じアーティスト名に揃える。
// track_artist はアップロード音声の artist タグ、artist は CD/AB 基本情報、
// authors はアプリ内で紐付けたアーティスト、album_artist は最後のフォールバック。
function getCdArtistIdentity(cd) {
    if (!cd || typeof cd !== "object") return null;

    const nonEmpty = (value) => {
        const text = String(value == null ? "" : value).trim();
        return text || null;
    };

    const trackArtist = nonEmpty(cd.track_artist);
    if (trackArtist) return { source: "track_artist", name: trackArtist };

    const artist = nonEmpty(cd.artist);
    if (artist) return { source: "artist", name: artist };

    const authors = Array.isArray(cd.authors) ? cd.authors : [];
    const primaryAuthor = authors
        .filter((author) => author && nonEmpty(author.name))
        .slice()
        .sort((a, b) => (Number(a.sort_order) || 0) - (Number(b.sort_order) || 0))[0];
    if (primaryAuthor) {
        return {
            source: "author",
            id: primaryAuthor.id,
            name: nonEmpty(primaryAuthor.name),
        };
    }

    const albumArtist = nonEmpty(cd.album_artist);
    return albumArtist ? { source: "album_artist", name: albumArtist } : null;
}

function getCdArtistName(cd) {
    return getCdArtistIdentity(cd)?.name || "";
}

function recordDataLoadError(key, error) {
    dataLoadErrors[key] = error instanceof Error ? error.message : String(error || "読み込みに失敗しました");
    console.error(`${key} load failed:`, error);
}

function clearDataLoadError(key) {
    delete dataLoadErrors[key];
}

function getDataLoadErrors() {
    return { ...dataLoadErrors };
}

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
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        allSeries = await res.json();
        clearDataLoadError("series");
        return true;
    } catch (error) {
        recordDataLoadError("series", error);
        allSeries = [];
        return false;
    }
}

async function loadGrandSeries() {
    try {
        const res = await fetch("/api/grand-series");
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        allGrandSeries = await res.json();
        clearDataLoadError("grand-series");
        return true;
    } catch (error) {
        recordDataLoadError("grand-series", error);
        allGrandSeries = [];
        return false;
    }
}

async function loadBooks() {
    try {
        const res = await fetch("/api/books");
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        allBooks = await res.json();
        clearDataLoadError("books");
        return true;
    } catch (err) {
        recordDataLoadError("books", err);
        allBooks = [];
        return false;
    }
}

async function loadCds() {
    try {
        const res = await fetch("/api/cds");
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        allCds = await res.json();
        clearDataLoadError("cds");
        return true;
    } catch (err) {
        recordDataLoadError("cds", err);
        allCds = [];
        return false;
    }
}

async function loadStorageLocations() {
    try {
        const res = await fetch("/api/storage-locations");
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        allStorageLocations = await res.json();
        clearDataLoadError("storage-locations");
        return true;
    } catch (error) {
        recordDataLoadError("storage-locations", error);
        allStorageLocations = [];
        return false;
    }
}

async function loadLabels() {
    try {
        const res = await fetch("/api/labels");
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        allLabels = await res.json();
        clearDataLoadError("labels");
        return true;
    } catch (error) {
        recordDataLoadError("labels", error);
        allLabels = [];
        return false;
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
    const previouslyFocused = document.activeElement;

    let overlay = document.getElementById("confirm-overlay");
    if (!overlay) {
        overlay = document.createElement("div");
        overlay.id = "confirm-overlay";
        overlay.className = "confirm-overlay hidden";
        overlay.innerHTML = `
            <div class="confirm-box" role="alertdialog" aria-modal="true" aria-labelledby="confirm-message">
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
            if (previouslyFocused && typeof previouslyFocused.focus === "function") {
                previouslyFocused.focus();
            }
        }

        function onOk() { cleanup(); resolve(true); }
        function onCancel() { cleanup(); resolve(false); }
        function onBg(e) { if (e.target === overlay) { cleanup(); resolve(false); } }
        function onKey(e) {
            if (e.key === "Escape") {
                cleanup();
                resolve(false);
                return;
            }
            if (e.key !== "Tab") return;
            const focusable = [cancelBtn, okBtn].filter((element) => !element.disabled);
            if (focusable.length === 0) return;
            const first = focusable[0];
            const last = focusable[focusable.length - 1];
            if (e.shiftKey && document.activeElement === first) {
                e.preventDefault();
                last.focus();
            } else if (!e.shiftKey && document.activeElement === last) {
                e.preventDefault();
                first.focus();
            }
        }

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
