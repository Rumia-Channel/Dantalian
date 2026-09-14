async function createStorageLocation() {
    const input = document.getElementById("new-location-name");
    const parentSelect = document.getElementById("new-location-parent");
    if (!input) return;
    const name = input.value.trim();
    if (!name) return;

    const parentId = parentSelect && parentSelect.value ? parseInt(parentSelect.value, 10) : null;

    try {
        const res = await fetch("/api/storage-locations", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ name, parent_id: parentId }),
        });
        if (res.ok) {
            input.value = "";
            if (parentSelect) parentSelect.value = "";
            await loadStorageLocations();
            renderStorageLocations();
        }
    } catch {}
}

async function renameStorageLocation(id) {
    const loc = allStorageLocations.find((x) => x.id === id);
    if (!loc) return;
    const newName = prompt("新しい名前", loc.name);
    if (newName === null || newName.trim() === "") return;

    try {
        const res = await fetch(`/api/storage-locations/${id}`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ name: newName.trim() }),
        });
        if (res.ok) {
            await loadStorageLocations();
            renderStorageLocations();
        }
    } catch {}
}

async function deleteStorageLocation(id) {
    const loc = allStorageLocations.find((x) => x.id === id);
    if (!loc) return;
    const path = getStorageLocationPath(id);
    const ok = await showConfirm({ message: `保管場所「${path}」を削除しますか？`, okLabel: "削除" });
    if (!ok) return;

    try {
        const res = await fetch(`/api/storage-locations/${id}`, { method: "DELETE" });
        if (res.ok) {
            await loadStorageLocations();
            renderStorageLocations();
        }
    } catch {}
}

function renderStorageLocations() {
    const list = document.getElementById("storage-location-list");
    if (!list) return;

    const parentSelect = document.getElementById("new-location-parent");
    if (parentSelect) {
        const currentVal = parentSelect.value;
        parentSelect.innerHTML = '<option value="">親なし (トップレベル)</option>' +
            allStorageLocations.map((l) =>
                `<option value="${l.id}">${escapeHtml(getStorageLocationPath(l.id))}</option>`
            ).join("");
        parentSelect.value = currentVal;
    }

    if (allStorageLocations.length === 0) {
        list.innerHTML = '<p class="series-empty">保管場所が登録されていません</p>';
        return;
    }

    const topLevel = allStorageLocations.filter((l) => l.parent_id == null);
    const children = allStorageLocations.filter((l) => l.parent_id != null);

    let html = "";
    for (const parent of topLevel) {
        const kids = children.filter((c) => c.parent_id === parent.id);
        html += `
        <div class="series-item">
            <span class="series-item-name">${escapeHtml(parent.name)}</span>
            <div class="series-item-actions">
                <button class="icon-btn" onclick="renameStorageLocation(${parent.id})" aria-label="名前変更" title="名前変更"><span class="material-icons" aria-hidden="true">edit</span></button>
                <button class="icon-btn icon-btn-danger" onclick="deleteStorageLocation(${parent.id})" aria-label="削除" title="削除"><span class="material-icons" aria-hidden="true">delete</span></button>
            </div>
        </div>`;
        for (const child of kids) {
            html += `
            <div class="series-item series-item-child">
                <span class="series-item-name series-item-child-name">${escapeHtml(child.name)}</span>
                <div class="series-item-actions">
                    <button class="icon-btn" onclick="renameStorageLocation(${child.id})" aria-label="名前変更" title="名前変更"><span class="material-icons" aria-hidden="true">edit</span></button>
                    <button class="icon-btn icon-btn-danger" onclick="deleteStorageLocation(${child.id})" aria-label="削除" title="削除"><span class="material-icons" aria-hidden="true">delete</span></button>
                </div>
            </div>`;
        }
    }

    const orphaned = children.filter((c) => !topLevel.some((t) => t.id === c.parent_id));
    for (const loc of orphaned) {
        html += `
        <div class="series-item">
            <span class="series-item-name">${escapeHtml(loc.name)}</span>
            <div class="series-item-actions">
                <button class="icon-btn" onclick="renameStorageLocation(${loc.id})" aria-label="名前変更" title="名前変更"><span class="material-icons" aria-hidden="true">edit</span></button>
                <button class="icon-btn icon-btn-danger" onclick="deleteStorageLocation(${loc.id})" aria-label="削除" title="削除"><span class="material-icons" aria-hidden="true">delete</span></button>
            </div>
        </div>`;
    }

    list.innerHTML = html;
}

document.addEventListener("DOMContentLoaded", () => {
    const input = document.getElementById("new-location-name");
    if (input) {
        input.addEventListener("keydown", (e) => {
            if (e.key === "Enter") {
                e.preventDefault();
                createStorageLocation();
            }
        });
    }
});
