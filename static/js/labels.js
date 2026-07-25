async function createLabel() {
    const input = document.getElementById("new-label-name");
    if (!input) return;
    const name = input.value.trim();
    if (!name) return;

    try {
        const res = await fetch("/api/labels", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ name }),
        });
        if (res.ok) {
            input.value = "";
            await loadLabels();
            renderLabels();
        }
    } catch {}
}

async function renameLabel(id) {
    const label = allLabels.find((x) => x.id === id);
    if (!label) return;
    const newName = prompt("新しい名前", label.name);
    if (newName === null || newName.trim() === "") return;

    try {
        const res = await fetch(`/api/labels/${id}`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ name: newName.trim() }),
        });
        if (res.ok) {
            await loadLabels();
            renderLabels();
        }
    } catch {}
}

async function deleteLabel(id) {
    const label = allLabels.find((x) => x.id === id);
    if (!label) return;
    const ok = await showConfirm({ message: `レーベル「${label.name}」を削除しますか？`, okLabel: "削除" });
    if (!ok) return;

    try {
        const res = await fetch(`/api/labels/${id}`, { method: "DELETE" });
        if (res.ok) {
            await loadLabels();
            renderLabels();
        }
    } catch {}
}

function renderLabels() {
    const list = document.getElementById("label-list");
    if (!list) return;

    if (allLabels.length === 0) {
        list.innerHTML = '<p class="series-empty">レーベルが登録されていません</p>';
        return;
    }

    list.innerHTML = allLabels.map((l) => `
        <div class="series-item">
            <span class="series-item-name">${escapeHtml(l.name)}</span>
            <div class="series-item-actions">
                <button class="btn btn-xs btn-ghost" onclick="renameLabel(${l.id})">名前変更</button>
                <button class="btn btn-xs btn-outline-danger" onclick="deleteLabel(${l.id})">削除</button>
            </div>
        </div>
    `).join("");
}

document.addEventListener("DOMContentLoaded", () => {
    const input = document.getElementById("new-label-name");
    if (input) {
        input.addEventListener("keydown", (e) => {
            if (e.key === "Enter") {
                e.preventDefault();
                createLabel();
            }
        });
    }
});
