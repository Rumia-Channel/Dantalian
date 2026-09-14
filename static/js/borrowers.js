let borrowers = [];

async function loadBorrowers() {
    try {
        const res = await fetch("/api/borrowers");
        if (res.ok) borrowers = await res.json();
    } catch {}
}

async function createBorrower() {
    const input = document.getElementById("new-borrower-name");
    if (!input) return;
    const name = input.value.trim();
    if (!name) return;

    try {
        const res = await fetch("/api/borrowers", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ name }),
        });
        if (res.ok) {
            input.value = "";
            await loadBorrowers();
            renderBorrowerList();
        }
    } catch {}
}

async function renameBorrower(id) {
    const b = borrowers.find((x) => x.id === id);
    if (!b) return;
    const newName = prompt("新しい名前", b.name);
    if (newName === null || newName.trim() === "") return;

    try {
        const res = await fetch(`/api/borrowers/${id}`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ name: newName.trim() }),
        });
        if (res.ok) {
            await loadBorrowers();
            renderBorrowerList();
        }
    } catch {}
}

async function deleteBorrower(id) {
    const b = borrowers.find((x) => x.id === id);
    if (!b) return;
    const ok = await showConfirm({ message: `借り手「${b.name}」を削除しますか？`, okLabel: "削除" });
    if (!ok) return;

    try {
        const res = await fetch(`/api/borrowers/${id}`, { method: "DELETE" });
        if (res.ok) {
            await loadBorrowers();
            renderBorrowerList();
        }
    } catch {}
}

function renderBorrowerList() {
    const list = document.getElementById("borrower-list");
    if (!list) return;

    if (borrowers.length === 0) {
        list.innerHTML = '<p class="series-empty">借り手が登録されていません</p>';
        return;
    }

    list.innerHTML = borrowers.map((b) => `
        <div class="series-item">
            <span class="series-item-name">${escapeHtml(b.name)}</span>
            ${b.notes ? `<span class="borrower-notes">${escapeHtml(b.notes)}</span>` : ""}
            <div class="series-item-actions">
                <button class="icon-btn" onclick="renameBorrower(${b.id})" aria-label="名前変更" title="名前変更"><span class="material-icons" aria-hidden="true">edit</span></button>
                <button class="icon-btn icon-btn-danger" onclick="deleteBorrower(${b.id})" aria-label="削除" title="削除"><span class="material-icons" aria-hidden="true">delete</span></button>
            </div>
        </div>
    `).join("");
}

document.addEventListener("DOMContentLoaded", () => {
    const input = document.getElementById("new-borrower-name");
    if (input) {
        input.addEventListener("keydown", (e) => {
            if (e.key === "Enter") {
                e.preventDefault();
                createBorrower();
            }
        });
    }
});
