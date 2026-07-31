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

    const parentBookId = document.getElementById("cd-parent-book-id");
    const parentId = parentBookId ? parseInt(parentBookId.value) || null : null;

    cdBtn.disabled = true;
    cdStatus.textContent = "検索中...";
    cdStatus.className = "";

    try {
        const body = { jan };
        if (parentId) body.parent_book_id = parentId;
        const res = await fetch("/api/cds", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
        });
        const data = await res.json();

        if (res.status === 300 && data.code === "musicbrainz_candidates") {
            openMusicBrainzCandidatePicker({
                jan,
                parentBookId: parentId,
                amazonTitle: data.amazon_title,
                candidates: data.candidates,
                onRegistered: (registered) => {
                    cdStatus.textContent = `「${registered.cd?.title || registered.title}」をMusicBrainzから登録しました`;
                    cdStatus.className = "success";
                    cdInput.value = "";
                },
            });
            return;
        }

        if (!res.ok) {
            cdStatus.textContent = data.error || "登録に失敗しました";
            cdStatus.className = "error";
            return;
        }

        const sourceLabel = data.cd ? "MusicBrainz" : "キャッシュ";
        cdStatus.textContent = `「${data.cd?.title || data.title}」を${sourceLabel}から登録しました`;
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
            const res = await fetch("/api/cds", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ jan: item.jan }),
            });
            const data = await res.json();

            if (!res.ok) {
                item.status = "error";
                item.error = data.code === "musicbrainz_candidates"
                    ? "候補が複数あります（単体登録で選択してください）"
                    : (data.error || "登録失敗");
                cdQueueFailed++;
            } else {
                item.status = "success";
                item.title = data.cd?.title || data.title;
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
