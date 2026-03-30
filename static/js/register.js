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

        const sourceLabel = data.source === "openbd" ? "OpenBD" : data.source === "ndl" ? "国立国会図書館" : "キャッシュ";
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

// --- Tab switching ---
const tabs = document.querySelectorAll(".register-tab");
const singlePanel = document.getElementById("single-panel");
const bulkPanel = document.getElementById("bulk-panel");

tabs.forEach((tab) => {
    tab.addEventListener("click", () => {
        tabs.forEach((t) => t.classList.remove("active"));
        tab.classList.add("active");
        const target = tab.dataset.tab;
        singlePanel.hidden = target !== "single";
        bulkPanel.hidden = target !== "bulk";
        if (target === "bulk") {
            document.getElementById("bulk-isbn-input").focus();
        }
    });
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
