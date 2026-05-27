async function renderCopiesSection(bookId) {
    const section = document.getElementById("edit-copies-section");
    if (!section) return;

    let copies = [];
    try {
        const res = await fetch(`/api/books/${bookId}/copies`);
        if (res.ok) copies = await res.json();
    } catch {}

    let html = `<h3 class="edit-section-title">所蔵管理 (${copies.length}件)</h3>`;

    copies.forEach((c) => {
        const isLent = !!c.lent_to;
        html += `
        <div class="edit-copy-item${isLent ? ' copy-lent' : ''}">
            <div class="edit-copy-info">
                <div class="edit-copy-main">
                    <span class="copy-type-icon">${c.copy_type === 'ebook' ? 'smartphone' : 'menu_book'}</span>
                    <span class="edit-copy-location">${c.location ? escapeHtml(c.location) : '<span class="copy-no-location">場所未設定</span>'}</span>
                    ${isLent
                        ? `<span class="copy-lent-badge">貸出中: ${escapeHtml(c.lent_to)}</span>`
                        : '<span class="copy-available-badge">所持</span>'}
                    ${c.due_date ? `<span class="copy-due-date">返却予定: ${escapeHtml(c.due_date)}</span>` : ''}
                </div>
                ${c.notes ? `<div class="edit-copy-notes">${escapeHtml(c.notes)}</div>` : ''}
            </div>
            <div class="edit-copy-actions">
                ${isLent
                    ? `<button type="button" class="btn btn-xs btn-outline-success" onclick="returnCopy(${c.id}, ${bookId})">返却</button>`
                    : `<button type="button" class="btn btn-xs btn-outline-warning" onclick="showLendForm(${c.id}, ${bookId})">貸出</button>`}
                <button type="button" class="btn btn-xs btn-ghost" onclick="editCopyDialog(${c.id}, ${c.copy_type}, ${JSON.stringify(c.location).replace(/"/g, '&quot;')}, ${JSON.stringify(c.notes).replace(/"/g, '&quot;')}, ${bookId})">編集</button>
                <button type="button" class="btn btn-xs btn-outline-danger" onclick="deleteCopy(${c.id}, ${bookId})">削除</button>
            </div>
        </div>`;
    });

    html += `
        <div class="edit-copy-add" id="edit-copy-add">
            <button type="button" class="btn btn-xs btn-outline-success" onclick="addCopy(${bookId})">+ 所蔵を追加</button>
        </div>
    `;

    html += `<div id="edit-lend-form-container"></div>`;

    section.innerHTML = html;
}

async function addCopy(bookId) {
    try {
        const res = await fetch(`/api/books/${bookId}/copies`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ copy_type: "physical" }),
        });
        if (res.ok) {
            await loadBooks();
            renderCopiesSection(bookId);
        }
    } catch {}
}

async function editCopyDialog(copyId, copyType, location, notes, bookId) {
    const newLocation = prompt("場所", location || "");
    if (newLocation === null) return;
    const newNotes = prompt("メモ", notes || "");
    if (newNotes === null) return;

    try {
        const res = await fetch(`/api/copies/${copyId}`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
                location: newLocation || null,
                notes: newNotes || null,
            }),
        });
        if (res.ok) {
            await loadBooks();
            renderCopiesSection(bookId);
        }
    } catch {}
}

async function deleteCopy(copyId, bookId) {
    const ok = await showConfirm({ message: "この所蔵を削除しますか？", okLabel: "削除" });
    if (!ok) return;

    try {
        const res = await fetch(`/api/copies/${copyId}`, { method: "DELETE" });
        if (res.ok) {
            await loadBooks();
            renderCopiesSection(bookId);
        }
    } catch {}
}

function showLendForm(copyId, bookId) {
    const container = document.getElementById("edit-lend-form-container");
    if (!container) return;

    const borrowerOpts = allBorrowers.map((b) => `<option value="${b.id}">${escapeHtml(b.name)}</option>`).join("");

    container.innerHTML = `
        <div class="edit-lend-form">
            <div class="edit-lend-title">貸出</div>
            <div class="edit-lend-fields">
                <select id="lend-borrower-select">
                    <option value="">借り手を選択...</option>
                    ${borrowerOpts}
                </select>
                <input type="date" id="lend-due-date" placeholder="返却予定日">
                <button type="button" class="btn btn-sm btn-primary" onclick="doLendCopy(${copyId}, ${bookId})">貸出実行</button>
                <button type="button" class="btn btn-sm btn-ghost" onclick="cancelLend()">キャンセル</button>
            </div>
        </div>
    `;
}

function cancelLend() {
    const container = document.getElementById("edit-lend-form-container");
    if (container) container.innerHTML = "";
}

async function doLendCopy(copyId, bookId) {
    const borrowerId = parseInt(document.getElementById("lend-borrower-select").value, 10);
    if (!borrowerId) return;

    const dueDateInput = document.getElementById("lend-due-date");
    const dueDate = dueDateInput ? dueDateInput.value || null : null;

    try {
        const res = await fetch(`/api/copies/${copyId}/lend`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ borrower_id: borrowerId, due_date: dueDate }),
        });
        if (res.ok) {
            await loadBooks();
            renderCopiesSection(bookId);
        }
    } catch {}
}

async function returnCopy(copyId, bookId) {
    const ok = await showConfirm({ message: "返却しますか？", okLabel: "返却" });
    if (!ok) return;

    try {
        const res = await fetch(`/api/copies/${copyId}/return`, { method: "POST" });
        if (res.ok) {
            await loadBooks();
            renderCopiesSection(bookId);
        }
    } catch {}
}
