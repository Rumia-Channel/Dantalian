// --- Tab switching ---
const tabs = document.querySelectorAll(".register-tab");
const isbnPanel = document.getElementById("isbn-panel");
const manualPanel = document.getElementById("manual-panel");

tabs.forEach((tab) => {
    tab.addEventListener("click", () => {
        tabs.forEach((t) => t.classList.remove("active"));
        tab.classList.add("active");
        const target = tab.dataset.tab;
        isbnPanel.hidden = target !== "isbn";
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
            <div class="edit-row">
                <div class="edit-field">
                    <label>ISBN <span class="edit-required">*</span></label>
                    <input type="text" name="isbn" required>
                </div>
            </div>
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
                    <label>出版社</label>
                    <input type="text" name="publisher">
                </div>
                <div class="edit-field">
                    <label>出版日</label>
                    <input type="text" name="publish_date">
                </div>
            </div>
            <div class="edit-row">
                <div class="edit-field">
                    <label>価格</label>
                    <input type="text" name="price">
                </div>
                <div class="edit-field">
                    <label>ページ数</label>
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
                <label>シリーズ名</label>
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
            <div class="edit-row">
                <div class="edit-field">
                    <label>JPNO</label>
                    <input type="text" name="jpno">
                </div>
                <div class="edit-field">
                    <label>NDL URL</label>
                    <input type="text" name="ndl_url">
                </div>
            </div>
            <div class="edit-field">
                <label>説明</label>
                <textarea name="description" rows="6"></textarea>
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
