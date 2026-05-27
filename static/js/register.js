// --- Tab switching ---
const tabs = document.querySelectorAll(".register-tab");
const isbnPanel = document.getElementById("isbn-panel");
const isdnPanel = document.getElementById("isdn-panel");
const manualPanel = document.getElementById("manual-panel");

const cdPanel = document.getElementById("cd-panel");
const audiobookPanel = document.getElementById("audiobook-panel");

tabs.forEach((tab) => {
    tab.addEventListener("click", () => {
        tabs.forEach((t) => t.classList.remove("active"));
        tab.classList.add("active");
        const target = tab.dataset.tab;
        isbnPanel.hidden = target !== "isbn";
        isdnPanel.hidden = target !== "isdn";
        cdPanel.hidden = target !== "cd";
        audiobookPanel.hidden = target !== "audiobook";
        manualPanel.hidden = target !== "manual";
        if (target === "manual") {
            renderManualForm();
        }
    });
});

// --- ISBN single registration ---
const registerForm = document.getElementById("register-form");
const isbnInput = document.getElementById("isbn-input");
const registerBtn = document.getElementById("register-btn");
const registerStatus = document.getElementById("register-status");

isbnInput.addEventListener("input", () => {
    isbnInput.value = isbnInput.value
        .replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xFEE0))
        .replace(/[\s\u3000\-－ー]/g, "");
});

registerForm.addEventListener("submit", async (e) => {
    e.preventDefault();
    const isbn = isbnInput.value.trim().replace(/-/g, "");
    if (!isbn) return;

    registerBtn.disabled = true;
    registerStatus.textContent = "検索中...";
    registerStatus.className = "";

    try {
        const res = await fetch("/api/books", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ isbn }),
        });
        const data = await res.json();

        if (!res.ok) {
            registerStatus.textContent = data.error || "登録に失敗しました";
            registerStatus.className = "error";
            return;
        }

        const sourceLabel = data.source === "amazon" ? "Amazon" : data.source === "ndl" ? "国立国会図書館" : data.source === "manual" ? "手動" : "キャッシュ";
        registerStatus.textContent = `「${data.book.title}」を${sourceLabel}から登録しました`;
        registerStatus.className = "success";
        isbnInput.value = "";
    } catch (err) {
        registerStatus.textContent = "通信エラーが発生しました";
        registerStatus.className = "error";
    } finally {
        registerBtn.disabled = false;
    }
});

// --- Bulk toggle ---
const bulkToggleBtn = document.getElementById("bulk-toggle-btn");
const bulkSection = document.getElementById("bulk-section");

bulkToggleBtn.addEventListener("click", () => {
    const isOpen = !bulkSection.hidden;
    bulkSection.hidden = isOpen;
    bulkToggleBtn.textContent = isOpen ? "一括登録を開く" : "一括登録を閉じる";
    if (!isOpen) {
        document.getElementById("bulk-isbn-input").focus();
    }
});

// --- Bulk registration queue ---
const bulkIsbnInput = document.getElementById("bulk-isbn-input");
const bulkClearBtn = document.getElementById("bulk-clear-btn");
const bulkStartBtn = document.getElementById("bulk-start-btn");
const bulkStopBtn = document.getElementById("bulk-stop-btn");
const bulkProgress = document.getElementById("bulk-progress");
const bulkQueueList = document.getElementById("bulk-queue");
const bulkEmpty = document.getElementById("bulk-empty");

let queue = [];
let queueRunning = false;
let queueStopRequested = false;
let queueDone = 0;
let queueFailed = 0;

function formatIsbn(val) {
    return val
        .replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xFEE0))
        .replace(/[\s\u3000\-－ー]/g, "");
}

bulkIsbnInput.addEventListener("input", () => {
    bulkIsbnInput.value = formatIsbn(bulkIsbnInput.value);
});

bulkIsbnInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
        e.preventDefault();
        const isbn = bulkIsbnInput.value.trim();
        if (!isbn) return;

        const dup = queue.some(
            (item) => item.isbn === isbn && (item.status === "pending" || item.status === "processing")
        );
        if (dup) {
            bulkIsbnInput.value = "";
            return;
        }

        queue.push({ isbn, status: "pending", title: null, error: null });
        bulkIsbnInput.value = "";
        renderBulkQueue();

        if (!queueRunning) {
            bulkStartBtn.disabled = false;
        }
    }
});

bulkStartBtn.addEventListener("click", startQueue);
bulkStopBtn.addEventListener("click", () => { queueStopRequested = true; });
bulkClearBtn.addEventListener("click", () => {
    if (queueRunning) return;
    queue = [];
    queueDone = 0;
    queueFailed = 0;
    renderBulkQueue();
    bulkProgress.textContent = "";
});

function renderBulkQueue() {
    bulkQueueList.innerHTML = "";
    bulkEmpty.hidden = queue.length > 0;

    queue.forEach((item, idx) => {
        const li = document.createElement("li");
        li.className = "bulk-item bulk-item--" + item.status;

        let statusIcon = "";
        if (item.status === "pending") statusIcon = "\u23F3";
        else if (item.status === "processing") statusIcon = "\uD83D\uDD04";
        else if (item.status === "success") statusIcon = "\u2713";
        else if (item.status === "error") statusIcon = "\u2717";

        const isbnSpan = document.createElement("span");
        isbnSpan.className = "bulk-item-isbn";
        isbnSpan.textContent = item.isbn;

        const titleSpan = document.createElement("span");
        titleSpan.className = "bulk-item-title";
        if (item.title) titleSpan.textContent = item.title;
        else if (item.error) titleSpan.textContent = item.error;
        else titleSpan.textContent = "待機中";

        li.appendChild(document.createTextNode(statusIcon + " "));
        li.appendChild(isbnSpan);
        li.appendChild(titleSpan);

        if (item.status === "pending" && !queueRunning) {
            const removeBtn = document.createElement("button");
            removeBtn.className = "btn btn-xs btn-ghost bulk-item-remove";
            removeBtn.textContent = "\u00D7";
            removeBtn.addEventListener("click", () => {
                queue.splice(idx, 1);
                renderBulkQueue();
                bulkStartBtn.disabled = queue.length === 0;
            });
            li.appendChild(removeBtn);
        }

        bulkQueueList.appendChild(li);
    });

    const total = queue.length;
    const finished = queueDone + queueFailed;
    if (finished > 0) {
        bulkProgress.textContent = finished + " / " + total + "（成功: " + queueDone + "、失敗: " + queueFailed + "）";
    }
}

async function startQueue() {
    if (queueRunning) return;
    queueRunning = true;
    queueStopRequested = false;
    queueDone = 0;
    queueFailed = 0;

    bulkStartBtn.hidden = true;
    bulkStopBtn.hidden = false;
    bulkClearBtn.disabled = true;
    bulkIsbnInput.disabled = true;

    for (let i = 0; i < queue.length; i++) {
        if (queueStopRequested) break;
        const item = queue[i];
        if (item.status !== "pending") continue;

        item.status = "processing";
        renderBulkQueue();

        try {
            const res = await fetch("/api/books", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ isbn: item.isbn }),
            });
            const data = await res.json();

            if (!res.ok) {
                item.status = "error";
                item.error = data.error || "登録失敗";
                queueFailed++;
            } else {
                item.status = "success";
                item.title = data.book.title;
                queueDone++;
            }
        } catch (err) {
            item.status = "error";
            item.error = "通信エラー";
            queueFailed++;
        }

        renderBulkQueue();
    }

    queueRunning = false;
    bulkStartBtn.hidden = false;
    bulkStopBtn.hidden = true;
    bulkClearBtn.disabled = false;
    bulkIsbnInput.disabled = false;
    bulkIsbnInput.focus();

    const hasPending = queue.some((item) => item.status === "pending");
    bulkStartBtn.disabled = !hasPending;
    bulkStartBtn.textContent = hasPending ? "再開" : "開始";
}

// --- ISDN single registration ---
const isdnForm = document.getElementById("isdn-form");
const isdnInput = document.getElementById("isdn-input");
const isdnBtn = document.getElementById("isdn-btn");
const isdnStatus = document.getElementById("isdn-status");

isdnInput.addEventListener("input", () => {
    isdnInput.value = isdnInput.value
        .replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xFEE0))
        .replace(/[\s\u3000\-－ー]/g, "");
});

isdnForm.addEventListener("submit", async (e) => {
    e.preventDefault();
    const isdn = isdnInput.value.trim().replace(/-/g, "");
    if (!isdn) return;

    isdnBtn.disabled = true;
    isdnStatus.textContent = "検索中...";
    isdnStatus.className = "";

    try {
        const res = await fetch("/api/books/isdn", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ isdn }),
        });
        const data = await res.json();

        if (!res.ok) {
            isdnStatus.textContent = data.error || "登録に失敗しました";
            isdnStatus.className = "error";
            return;
        }

        const sourceLabel = data.source === "isdn" ? "ISDN" : data.source === "cache" ? "キャッシュ" : data.source;
        isdnStatus.textContent = `「${data.book.title}」を${sourceLabel}から登録しました`;
        isdnStatus.className = "success";
        isdnInput.value = "";
    } catch (err) {
        isdnStatus.textContent = "通信エラーが発生しました";
        isdnStatus.className = "error";
    } finally {
        isdnBtn.disabled = false;
    }
});

// --- ISDN bulk toggle ---
const isdnBulkToggleBtn = document.getElementById("isdn-bulk-toggle-btn");
const isdnBulkSection = document.getElementById("isdn-bulk-section");

isdnBulkToggleBtn.addEventListener("click", () => {
    const isOpen = !isdnBulkSection.hidden;
    isdnBulkSection.hidden = isOpen;
    isdnBulkToggleBtn.textContent = isOpen ? "一括登録を開く" : "一括登録を閉じる";
    if (!isOpen) {
        document.getElementById("isdn-bulk-input").focus();
    }
});

// --- ISDN bulk registration queue ---
const isdnBulkInput = document.getElementById("isdn-bulk-input");
const isdnBulkClearBtn = document.getElementById("isdn-bulk-clear-btn");
const isdnBulkStartBtn = document.getElementById("isdn-bulk-start-btn");
const isdnBulkStopBtn = document.getElementById("isdn-bulk-stop-btn");
const isdnBulkProgress = document.getElementById("isdn-bulk-progress");
const isdnBulkQueueList = document.getElementById("isdn-bulk-queue");
const isdnBulkEmpty = document.getElementById("isdn-bulk-empty");

let isdnQueue = [];
let isdnQueueRunning = false;
let isdnQueueStopRequested = false;
let isdnQueueDone = 0;
let isdnQueueFailed = 0;

function formatIsdn(val) {
    return val
        .replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xFEE0))
        .replace(/[\s\u3000\-－ー]/g, "");
}

isdnBulkInput.addEventListener("input", () => {
    isdnBulkInput.value = formatIsdn(isdnBulkInput.value);
});

isdnBulkInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
        e.preventDefault();
        const isdn = isdnBulkInput.value.trim();
        if (!isdn) return;

        const dup = isdnQueue.some(
            (item) => item.isdn === isdn && (item.status === "pending" || item.status === "processing")
        );
        if (dup) {
            isdnBulkInput.value = "";
            return;
        }

        isdnQueue.push({ isdn, status: "pending", title: null, error: null });
        isdnBulkInput.value = "";
        renderIsdnBulkQueue();

        if (!isdnQueueRunning) {
            isdnBulkStartBtn.disabled = false;
        }
    }
});

isdnBulkStartBtn.addEventListener("click", startIsdnQueue);
isdnBulkStopBtn.addEventListener("click", () => { isdnQueueStopRequested = true; });
isdnBulkClearBtn.addEventListener("click", () => {
    if (isdnQueueRunning) return;
    isdnQueue = [];
    isdnQueueDone = 0;
    isdnQueueFailed = 0;
    renderIsdnBulkQueue();
    isdnBulkProgress.textContent = "";
});

function renderIsdnBulkQueue() {
    isdnBulkQueueList.innerHTML = "";
    isdnBulkEmpty.hidden = isdnQueue.length > 0;

    isdnQueue.forEach((item, idx) => {
        const li = document.createElement("li");
        li.className = "bulk-item bulk-item--" + item.status;

        let statusIcon = "";
        if (item.status === "pending") statusIcon = "\u23F3";
        else if (item.status === "processing") statusIcon = "\uD83D\uDD04";
        else if (item.status === "success") statusIcon = "\u2713";
        else if (item.status === "error") statusIcon = "\u2717";

        const isdnSpan = document.createElement("span");
        isdnSpan.className = "bulk-item-isdn";
        isdnSpan.textContent = item.isdn;

        const titleSpan = document.createElement("span");
        titleSpan.className = "bulk-item-title";
        if (item.title) titleSpan.textContent = item.title;
        else if (item.error) titleSpan.textContent = item.error;
        else titleSpan.textContent = "待機中";

        li.appendChild(document.createTextNode(statusIcon + " "));
        li.appendChild(isdnSpan);
        li.appendChild(titleSpan);

        if (item.status === "pending" && !isdnQueueRunning) {
            const removeBtn = document.createElement("button");
            removeBtn.className = "btn btn-xs btn-ghost bulk-item-remove";
            removeBtn.textContent = "\u00D7";
            removeBtn.addEventListener("click", () => {
                isdnQueue.splice(idx, 1);
                renderIsdnBulkQueue();
                isdnBulkStartBtn.disabled = isdnQueue.length === 0;
            });
            li.appendChild(removeBtn);
        }

        isdnBulkQueueList.appendChild(li);
    });

    const total = isdnQueue.length;
    const finished = isdnQueueDone + isdnQueueFailed;
    if (finished > 0) {
        isdnBulkProgress.textContent = finished + " / " + total + "（成功: " + isdnQueueDone + "、失敗: " + isdnQueueFailed + "）";
    }
}

async function startIsdnQueue() {
    if (isdnQueueRunning) return;
    isdnQueueRunning = true;
    isdnQueueStopRequested = false;
    isdnQueueDone = 0;
    isdnQueueFailed = 0;

    isdnBulkStartBtn.hidden = true;
    isdnBulkStopBtn.hidden = false;
    isdnBulkClearBtn.disabled = true;
    isdnBulkInput.disabled = true;

    for (let i = 0; i < isdnQueue.length; i++) {
        if (isdnQueueStopRequested) break;
        const item = isdnQueue[i];
        if (item.status !== "pending") continue;

        item.status = "processing";
        renderIsdnBulkQueue();

        try {
            const res = await fetch("/api/books/isdn", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ isdn: item.isdn }),
            });
            const data = await res.json();

            if (!res.ok) {
                item.status = "error";
                item.error = data.error || "登録失敗";
                isdnQueueFailed++;
            } else {
                item.status = "success";
                item.title = data.book.title;
                isdnQueueDone++;
            }
        } catch (err) {
            item.status = "error";
            item.error = "通信エラー";
            isdnQueueFailed++;
        }

        renderIsdnBulkQueue();
    }

    isdnQueueRunning = false;
    isdnBulkStartBtn.hidden = false;
    isdnBulkStopBtn.hidden = true;
    isdnBulkClearBtn.disabled = false;
    isdnBulkInput.disabled = false;
    isdnBulkInput.focus();

    const hasPending = isdnQueue.some((item) => item.status === "pending");
    isdnBulkStartBtn.disabled = !hasPending;
    isdnBulkStartBtn.textContent = hasPending ? "再開" : "開始";
}

// --- Manual registration ---
let manualAllAuthors = [];
let manualAllSeries = [];
let manualAllGrandSeries = [];
let manualCoverFile = null;
let manualCoverPreview = null;
let manualAuthorIds = [];
let manualRendered = false;
let manualAuthorSelect = null;
let manualSeriesSelect = null;
let manualGrandSeriesSelect = null;

async function renderManualForm() {
    if (manualRendered) return;
    manualRendered = true;

    try {
        const [authorsRes, seriesRes, gsRes] = await Promise.all([
            fetch("/api/authors"),
            fetch("/api/series"),
            fetch("/api/grand-series"),
        ]);
        if (authorsRes.ok) manualAllAuthors = await authorsRes.json();
        if (seriesRes.ok) manualAllSeries = await seriesRes.json();
        if (gsRes.ok) manualAllGrandSeries = await gsRes.json();
    } catch {}

    const container = document.getElementById("manual-form-container");
    container.innerHTML = `
        <form class="edit-form" id="manual-form" onsubmit="submitManualBook(event)">
            <input type="hidden" name="series_id" value="">
            <input type="hidden" name="grand_series_id" value="">
            <div class="edit-field">
                <label>タイトル <span class="edit-required">*</span></label>
                <input type="text" name="title" required>
            </div>
            <div class="edit-field">
                <label>タイトル(よみ)</label>
                <input type="text" name="title_transcription">
            </div>
            <div class="edit-row">
                <div class="edit-field">
                    <label>出版社 / サークル名</label>
                    <input type="text" name="publisher">
                </div>
                <div class="edit-field">
                    <label>出版日 / 発行日</label>
                    <input type="text" name="publish_date">
                </div>
            </div>
            <div class="edit-row">
                <div class="edit-field">
                    <label>価格</label>
                    <input type="text" name="price">
                </div>
                <div class="edit-field">
                    <label>ページ数 / 体裁</label>
                    <input type="text" name="extent">
                </div>
            </div>
            <div class="edit-row">
                <div class="edit-field">
                    <label>巻</label>
                    <input type="text" name="volume">
                </div>
                <div class="edit-field">
                    <label>巻(よみ)</label>
                    <input type="text" name="volume_transcription">
                </div>
            </div>
            <div class="edit-field">
                <label>シリーズ名(NDL)</label>
                <input type="text" name="series_title">
            </div>
            <div class="edit-field">
                <label>シリーズ名(よみ)</label>
                <input type="text" name="series_title_transcription">
            </div>
            <div class="edit-field">
                <label>別タイトル</label>
                <input type="text" name="alternative">
            </div>
            <div class="edit-field">
                <label>別タイトル(よみ)</label>
                <input type="text" name="alternative_transcription">
            </div>
            <div class="edit-field">
                <label>説明</label>
                <textarea name="description" rows="6"></textarea>
            </div>
            <div class="edit-section">
                <h3 class="edit-section-title">ISBN / NDL 固有</h3>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>ISBN</label>
                        <input type="text" name="isbn">
                    </div>
                    <div class="edit-field">
                        <label>JPNO</label>
                        <input type="text" name="jpno">
                    </div>
                </div>
                <div class="edit-field">
                    <label>NDL URL</label>
                    <input type="text" name="ndl_url">
                </div>
            </div>
            <div class="edit-section">
                <h3 class="edit-section-title">ISDN 固有</h3>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>ISDN</label>
                        <input type="text" name="isdn">
                    </div>
                    <div class="edit-field">
                        <label>Cコード</label>
                        <input type="text" name="isdn_c_code">
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>区分</label>
                        <input type="text" name="isdn_class">
                    </div>
                    <div class="edit-field">
                        <label>形態</label>
                        <input type="text" name="isdn_type">
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>レーティング(性別)</label>
                        <input type="text" name="isdn_rating_gender">
                    </div>
                    <div class="edit-field">
                        <label>レーティング(年齢)</label>
                        <input type="text" name="isdn_rating_age">
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>地域</label>
                        <input type="text" name="isdn_region">
                    </div>
                    <div class="edit-field">
                        <label>ジャンルコード</label>
                        <input type="text" name="isdn_genre_code">
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>ジャンル名</label>
                        <input type="text" name="isdn_genre_name">
                    </div>
                    <div class="edit-field">
                        <label>ジャンル補足</label>
                        <input type="text" name="isdn_genre_user">
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>販売対象</label>
                        <input type="text" name="isdn_author">
                    </div>
                    <div class="edit-field">
                        <label>書籍形態(Cコード)</label>
                        <input type="text" name="isdn_shape">
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>内容(Cコード)</label>
                        <input type="text" name="isdn_contents">
                    </div>
                    <div class="edit-field">
                        <label>バーコード2段目</label>
                        <input type="text" name="isdn_barcode2">
                    </div>
                </div>
                <div class="edit-field">
                    <label>サンプル画像URL</label>
                    <input type="text" name="isdn_sample_image_url">
                </div>
            </div>
            <div class="edit-field">
                <label>表紙画像</label>
                <div class="manual-cover-row">
                    <label class="btn btn-xs btn-outline-success manual-cover-label">
                        ファイルを選択
                        <input type="file" id="manual-cover-input" accept="image/*" hidden>
                    </label>
                    <span class="manual-cover-filename" id="manual-cover-filename"></span>
                    ${manualCoverPreview ? `<img class="manual-cover-preview" id="manual-cover-preview" src="${manualCoverPreview}" alt="">` : '<img class="manual-cover-preview" id="manual-cover-preview" src="" alt="" hidden>'}
                </div>
            </div>
            <div class="edit-section">
                <h3 class="edit-section-title">作者</h3>
                <div class="edit-author-list" id="manual-author-list"></div>
                <div class="edit-author-add">
                    <div id="manual-author-select-container"></div>
                    <button type="button" class="btn btn-xs btn-outline-success" onclick="addManualAuthor()">追加</button>
                </div>
            </div>
            <div class="edit-section">
                <h3 class="edit-section-title">シリーズ設定</h3>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>シリーズ</label>
                        <div id="manual-series-select-container"></div>
                    </div>
                    <div class="edit-field">
                        <label>シリーズ巻数</label>
                        <input type="number" name="series_number" min="1" step="1">
                    </div>
                </div>
                <div class="edit-field">
                    <label>大シリーズ</label>
                    <div id="manual-grand-series-select-container"></div>
                </div>
            </div>
            <div id="manual-register-status"></div>
            <div class="edit-actions">
                <button type="submit" class="btn btn-md btn-primary">登録</button>
            </div>
        </form>
    `;

    const form = document.getElementById("manual-form");

    manualSeriesSelect = createSearchableSelect(document.getElementById("manual-series-select-container"), {
        options: manualAllSeries.map((s) => ({ value: s.id, label: s.name })),
        value: null,
        placeholder: "なし",
        onChange: (val) => {
            form.querySelector("input[name=series_id]").value = val != null ? val : "";
        },
    });

    manualGrandSeriesSelect = createSearchableSelect(document.getElementById("manual-grand-series-select-container"), {
        options: manualAllGrandSeries.map((gs) => ({ value: gs.id, label: gs.name })),
        value: null,
        placeholder: "なし",
        onChange: (val) => {
            form.querySelector("input[name=grand_series_id]").value = val != null ? val : "";
        },
    });

    manualAuthorSelect = createSearchableSelect(document.getElementById("manual-author-select-container"), {
        options: manualAllAuthors.map((a) => ({ value: a.id, label: a.name })),
        value: null,
        placeholder: "作者を追加...",
        clearable: false,
    });

    document.getElementById("manual-form").querySelector("input[name=isbn]").addEventListener("input", function () {
        this.value = this.value
            .replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xFEE0))
            .replace(/[\s\u3000\-－ー]/g, "");
    });

    const isdnField = document.getElementById("manual-form").querySelector("input[name=isdn]");
    if (isdnField) {
        isdnField.addEventListener("input", function () {
            this.value = this.value
                .replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xFEE0))
                .replace(/[\s\u3000\-－ー]/g, "");
        });
    }

    document.getElementById("manual-cover-input").addEventListener("change", (e) => {
        const file = e.target.files[0];
        if (!file) return;
        manualCoverFile = file;
        document.getElementById("manual-cover-filename").textContent = file.name;
        const reader = new FileReader();
        reader.onload = (ev) => {
            const img = document.getElementById("manual-cover-preview");
            img.src = ev.target.result;
            img.hidden = false;
        };
        reader.readAsDataURL(file);
    });
}

function renderManualAuthorList() {
    const list = document.getElementById("manual-author-list");
    if (!list) return;
    list.innerHTML = "";
    manualAuthorIds.forEach((aid, idx) => {
        const author = manualAllAuthors.find((a) => a.id === aid);
        if (!author) return;
        const div = document.createElement("div");
        div.className = "edit-author-item";
        div.innerHTML = `
            <div class="edit-author-info">
                <div class="edit-author-name">${escapeHtml(author.name)}</div>
                <div class="edit-author-meta">
                    ${author.transcription ? `<span class="edit-author-yomi">${escapeHtml(author.transcription)}</span>` : ""}
                    ${author.ndl_id ? `<span class="edit-author-ndl">NDL: ${escapeHtml(author.ndl_id)}</span>` : ""}
                </div>
            </div>
            <button type="button" class="btn btn-xs btn-outline-danger" data-idx="${idx}">削除</button>
        `;
        div.querySelector("button").addEventListener("click", () => {
            manualAuthorIds.splice(idx, 1);
            renderManualAuthorList();
        });
        list.appendChild(div);
    });
}

function addManualAuthor() {
    if (!manualAuthorSelect) return;
    const aid = manualAuthorSelect.getValue();
    if (!aid) return;
    if (manualAuthorIds.includes(aid)) return;
    manualAuthorIds.push(aid);
    manualAuthorSelect.setValue(null);
    renderManualAuthorList();
}

async function submitManualBook(e) {
    e.preventDefault();
    const form = e.target;
    const fd = new FormData(form);
    const body = {};
    for (const [key, val] of fd.entries()) {
        if (key === "series_id" || key === "grand_series_id" || key === "series_number") {
            body[key] = val === "" ? null : parseInt(val, 10);
        } else if (key === "isbn" || key === "isdn") {
            body[key] = val === "" ? null : val;
        } else {
            body[key] = val === "" ? null : val;
        }
    }
    body.author_ids = manualAuthorIds.length > 0 ? manualAuthorIds : undefined;

    const statusEl = document.getElementById("manual-register-status");

    try {
        const res = await fetch("/api/books/manual", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
        });
        const data = await res.json();

        if (!res.ok) {
            statusEl.textContent = data.error || "登録に失敗しました";
            statusEl.className = "error";
            return;
        }

        const bookId = data.book.id;

        if (manualCoverFile && bookId) {
            const coverFd = new FormData();
            coverFd.append("cover", manualCoverFile);
            await fetch(`/api/books/${bookId}/cover`, { method: "POST", body: coverFd });
        }

        statusEl.textContent = `「${data.book.title}」を登録しました`;
        statusEl.className = "success";
        manualCoverFile = null;
        manualCoverPreview = null;
        manualAuthorIds = [];
        manualRendered = false;
        manualAuthorSelect = null;
        manualSeriesSelect = null;
        manualGrandSeriesSelect = null;
        form.reset();
        const preview = document.getElementById("manual-cover-preview");
        if (preview) preview.hidden = true;
        const fname = document.getElementById("manual-cover-filename");
        if (fname) fname.textContent = "";
        renderManualAuthorList();
    } catch (err) {
        statusEl.textContent = "通信エラーが発生しました";
        statusEl.className = "error";
    }
}

// --- CD single registration ---
const cdForm = document.getElementById("cd-form");
const cdInput = document.getElementById("cd-input");
const cdBtn = document.getElementById("cd-btn");
const cdStatus = document.getElementById("cd-status");

cdInput.addEventListener("input", () => {
    cdInput.value = cdInput.value
        .replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xFEE0))
        .replace(/[\s\u3000\-－ー]/g, "");
});

cdForm.addEventListener("submit", async (e) => {
    e.preventDefault();
    const jan = cdInput.value.trim().replace(/-/g, "");
    if (!jan) return;

    cdBtn.disabled = true;
    cdStatus.textContent = "検索中...";
    cdStatus.className = "";

    try {
        const res = await fetch("/api/books/cd", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ jan }),
        });
        const data = await res.json();

        if (!res.ok) {
            cdStatus.textContent = data.error || "登録に失敗しました";
            cdStatus.className = "error";
            return;
        }

        const sourceLabel = data.source === "musicbrainz" ? "MusicBrainz" : "キャッシュ";
        cdStatus.textContent = `「${data.book.title}」を${sourceLabel}から登録しました`;
        cdStatus.className = "success";
        cdInput.value = "";
    } catch (err) {
        cdStatus.textContent = "通信エラーが発生しました";
        cdStatus.className = "error";
    } finally {
        cdBtn.disabled = false;
    }
});

// --- CD bulk registration queue ---
const cdBulkToggleBtn = document.getElementById("cd-bulk-toggle-btn");
const cdBulkSection = document.getElementById("cd-bulk-section");

cdBulkToggleBtn.addEventListener("click", () => {
    const isOpen = !cdBulkSection.hidden;
    cdBulkSection.hidden = isOpen;
    cdBulkToggleBtn.textContent = isOpen ? "一括登録を開く" : "一括登録を閉じる";
    if (!isOpen) {
        document.getElementById("cd-bulk-input").focus();
    }
});

const cdBulkInput = document.getElementById("cd-bulk-input");
const cdBulkClearBtn = document.getElementById("cd-bulk-clear-btn");
const cdBulkStartBtn = document.getElementById("cd-bulk-start-btn");
const cdBulkStopBtn = document.getElementById("cd-bulk-stop-btn");
const cdBulkProgress = document.getElementById("cd-bulk-progress");
const cdBulkQueueList = document.getElementById("cd-bulk-queue");
const cdBulkEmpty = document.getElementById("cd-bulk-empty");

let cdQueue = [];
let cdQueueRunning = false;
let cdQueueStopRequested = false;
let cdQueueDone = 0;
let cdQueueFailed = 0;

function formatJan(val) {
    return val
        .replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xFEE0))
        .replace(/[\s\u3000\-－ー]/g, "");
}

cdBulkInput.addEventListener("input", () => {
    cdBulkInput.value = formatJan(cdBulkInput.value);
});

cdBulkInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
        e.preventDefault();
        const jan = cdBulkInput.value.trim();
        if (!jan) return;

        const dup = cdQueue.some(
            (item) => item.jan === jan && (item.status === "pending" || item.status === "processing")
        );
        if (dup) {
            cdBulkInput.value = "";
            return;
        }

        cdQueue.push({ jan, status: "pending", title: null, error: null });
        cdBulkInput.value = "";
        renderCdBulkQueue();

        if (!cdQueueRunning) {
            cdBulkStartBtn.disabled = false;
        }
    }
});

cdBulkStartBtn.addEventListener("click", startCdQueue);
cdBulkStopBtn.addEventListener("click", () => { cdQueueStopRequested = true; });
cdBulkClearBtn.addEventListener("click", () => {
    if (cdQueueRunning) return;
    cdQueue = [];
    cdQueueDone = 0;
    cdQueueFailed = 0;
    renderCdBulkQueue();
    cdBulkProgress.textContent = "";
});

function renderCdBulkQueue() {
    cdBulkQueueList.innerHTML = "";
    cdBulkEmpty.hidden = cdQueue.length > 0;

    cdQueue.forEach((item, idx) => {
        const li = document.createElement("li");
        li.className = "bulk-item bulk-item--" + item.status;

        let statusIcon = "";
        if (item.status === "pending") statusIcon = "\u23F3";
        else if (item.status === "processing") statusIcon = "\uD83D\uDD04";
        else if (item.status === "success") statusIcon = "\u2713";
        else if (item.status === "error") statusIcon = "\u2717";

        const janSpan = document.createElement("span");
        janSpan.className = "bulk-item-isbn";
        janSpan.textContent = item.jan;

        const titleSpan = document.createElement("span");
        titleSpan.className = "bulk-item-title";
        if (item.title) titleSpan.textContent = item.title;
        else if (item.error) titleSpan.textContent = item.error;
        else titleSpan.textContent = "待機中";

        li.appendChild(document.createTextNode(statusIcon + " "));
        li.appendChild(janSpan);
        li.appendChild(titleSpan);

        if (item.status === "pending" && !cdQueueRunning) {
            const removeBtn = document.createElement("button");
            removeBtn.className = "btn btn-xs btn-ghost bulk-item-remove";
            removeBtn.textContent = "\u00D7";
            removeBtn.addEventListener("click", () => {
                cdQueue.splice(idx, 1);
                renderCdBulkQueue();
                cdBulkStartBtn.disabled = cdQueue.length === 0;
            });
            li.appendChild(removeBtn);
        }

        cdBulkQueueList.appendChild(li);
    });

    const total = cdQueue.length;
    const finished = cdQueueDone + cdQueueFailed;
    if (finished > 0) {
        cdBulkProgress.textContent = finished + " / " + total + "（成功: " + cdQueueDone + "、失敗: " + cdQueueFailed + "）";
    }
}

async function startCdQueue() {
    if (cdQueueRunning) return;
    cdQueueRunning = true;
    cdQueueStopRequested = false;
    cdQueueDone = 0;
    cdQueueFailed = 0;

    cdBulkStartBtn.hidden = true;
    cdBulkStopBtn.hidden = false;
    cdBulkClearBtn.disabled = true;
    cdBulkInput.disabled = true;

    for (let i = 0; i < cdQueue.length; i++) {
        if (cdQueueStopRequested) break;
        const item = cdQueue[i];
        if (item.status !== "pending") continue;

        item.status = "processing";
        renderCdBulkQueue();

        try {
            const res = await fetch("/api/books/cd", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ jan: item.jan }),
            });
            const data = await res.json();

            if (!res.ok) {
                item.status = "error";
                item.error = data.error || "登録失敗";
                cdQueueFailed++;
            } else {
                item.status = "success";
                item.title = data.book.title;
                cdQueueDone++;
            }
        } catch (err) {
            item.status = "error";
            item.error = "通信エラー";
            cdQueueFailed++;
        }

        renderCdBulkQueue();
    }

    cdQueueRunning = false;
    cdBulkStartBtn.hidden = false;
    cdBulkStopBtn.hidden = true;
    cdBulkClearBtn.disabled = false;
    cdBulkInput.disabled = false;
    cdBulkInput.focus();

    const hasPending = cdQueue.some((item) => item.status === "pending");
    cdBulkStartBtn.disabled = !hasPending;
    cdBulkStartBtn.textContent = hasPending ? "再開" : "開始";
}

// --- Audiobook registration ---
const audiobookForm = document.getElementById("audiobook-form");
const audiobookInput = document.getElementById("audiobook-input");
const audiobookBtn = document.getElementById("audiobook-btn");
const audiobookStatus = document.getElementById("audiobook-status");

audiobookInput.addEventListener("input", () => {
    audiobookInput.value = audiobookInput.value
        .replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xFEE0))
        .replace(/[\s\u3000\-－ー]/g, "");
});

audiobookForm.addEventListener("submit", async (e) => {
    e.preventDefault();
    const isbn = audiobookInput.value.trim().replace(/-/g, "");
    if (!isbn) return;

    audiobookBtn.disabled = true;
    audiobookStatus.textContent = "検索中...";
    audiobookStatus.className = "";

    try {
        const res = await fetch("/api/books", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ isbn, media_type: "audiobook" }),
        });
        const data = await res.json();

        if (!res.ok) {
            audiobookStatus.textContent = data.error || "登録に失敗しました";
            audiobookStatus.className = "error";
            return;
        }

        const sourceLabel = data.source === "amazon" ? "Amazon" : "国立国会図書館";
        audiobookStatus.textContent = `「${data.book.title}」を${sourceLabel}からオーディオブックとして登録しました`;
        audiobookStatus.className = "success";
        audiobookInput.value = "";
    } catch (err) {
        audiobookStatus.textContent = "通信エラーが発生しました";
        audiobookStatus.className = "error";
    } finally {
        audiobookBtn.disabled = false;
    }
});
