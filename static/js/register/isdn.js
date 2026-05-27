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
