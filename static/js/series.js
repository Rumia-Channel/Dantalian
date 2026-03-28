function switchSeriesTab(tab) {
    document.getElementById("tab-series").classList.toggle("active", tab === "series");
    document.getElementById("tab-grand-series").classList.toggle("active", tab === "grand-series");
    document.getElementById("panel-series").classList.toggle("hidden", tab !== "series");
    document.getElementById("panel-grand-series").classList.toggle("hidden", tab !== "grand-series");
    if (tab === "grand-series") renderGrandSeriesManager();
}

function renderSeriesManager() {
    const list = document.getElementById("series-list");
    if (allSeries.length === 0) {
        list.innerHTML = '<p style="color:#555;font-size:0.85rem;padding:0.5rem 0;">シリーズがありません</p>';
        return;
    }
    list.innerHTML = allSeries.map((s) => {
        const gs = findSeriesGrandSeries(s.id);
        const gsLabel = gs ? ` <span class="series-belong-grand">→ ${escapeHtml(gs.name)}</span>` : "";
        return `
        <div class="series-list-item" id="series-item-${s.id}">
            <span class="series-list-name" ondblclick="startRenameSeries(${s.id}, '${escapeAttr(s.name)}')">${escapeHtml(s.name)}${gsLabel}</span>
            <div class="series-list-actions">
                <button class="btn-rename" onclick="startRenameSeries(${s.id}, '${escapeAttr(s.name)}')">改名</button>
                <button class="btn-delete" onclick="deleteSeries(${s.id})">削除</button>
            </div>
        </div>`;
    }).join("");
}

function findSeriesGrandSeries(seriesId) {
    for (const gs of allGrandSeries) {
        if (gs.items.some((it) => it.item_type === "series" && it.item_id === seriesId)) return gs;
    }
    return null;
}

function findBookGrandSeries(bookId) {
    for (const gs of allGrandSeries) {
        if (gs.items.some((it) => it.item_type === "book" && it.item_id === bookId)) return gs;
    }
    return null;
}

async function createSeries() {
    const input = document.getElementById("new-series-name");
    const name = input.value.trim();
    if (!name) return;

    try {
        const res = await fetch("/api/series", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ name }),
        });
        if (res.ok) {
            input.value = "";
            await loadSeries();
            renderSeriesManager();
            loadBooks();
        }
    } catch {}
}

async function startRenameSeries(id, oldName) {
    const el = document.getElementById(`series-item-${id}`);
    if (!el) return;
    const nameEl = el.querySelector(".series-list-name");
    const actionsEl = el.querySelector(".series-list-actions");

    const input = document.createElement("input");
    input.className = "inline-edit-input";
    input.value = oldName;

    const save = async () => {
        const newName = input.value.trim();
        if (newName && newName !== oldName) {
            await fetch(`/api/series/${id}`, {
                method: "PUT",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ name: newName }),
            });
        }
        await loadSeries();
        renderSeriesManager();
        loadBooks();
    };

    const cancel = () => {
        nameEl.textContent = oldName;
        actionsEl.style.display = "";
    };

    input.addEventListener("keydown", (e) => {
        if (e.key === "Enter") save();
        if (e.key === "Escape") cancel();
    });
    input.addEventListener("blur", save);

    nameEl.textContent = "";
    nameEl.appendChild(input);
    actionsEl.style.display = "none";
    input.focus();
    input.select();
}

async function deleteSeries(id) {
    const s = allSeries.find((x) => x.id === id);
    if (!s) return;
    if (!confirm(`シリーズ「${s.name}」を削除しますか？\n所属している本はシリーズから外れます。`)) return;

    try {
        const res = await fetch(`/api/series/${id}`, { method: "DELETE" });
        if (res.ok) {
            await loadSeries();
            await loadGrandSeries();
            renderSeriesManager();
            loadBooks();
        }
    } catch {}
}

function renderGrandSeriesManager() {
    const list = document.getElementById("grand-series-list");
    if (allGrandSeries.length === 0) {
        list.innerHTML = '<p style="color:#555;font-size:0.85rem;padding:0.5rem 0;">大シリーズがありません</p>';
        return;
    }
    list.innerHTML = allGrandSeries.map((gs) => {
        const itemsHtml = gs.items.map((it) => {
            const typeLabel = it.item_type === "series" ? "シリーズ" : "書籍";
            return `<div class="gs-item">
                <span class="gs-item-type">${typeLabel}</span>
                <span class="gs-item-name">${escapeHtml(it.name)}</span>
                <button class="gs-item-remove" onclick="removeGrandSeriesItem(${gs.id}, '${it.item_type}', ${it.item_id})">×</button>
            </div>`;
        }).join("");

        return `
        <div class="gs-list-item" id="gs-item-${gs.id}">
            <div class="gs-list-header">
                <span class="gs-list-name" ondblclick="startRenameGrandSeries(${gs.id}, '${escapeAttr(gs.name)}')">${escapeHtml(gs.name)}</span>
                <div class="series-list-actions">
                    <button class="btn-rename" onclick="startRenameGrandSeries(${gs.id}, '${escapeAttr(gs.name)}')">改名</button>
                    <button class="btn-delete" onclick="deleteGrandSeries(${gs.id})">削除</button>
                </div>
            </div>
            <div class="gs-items">${itemsHtml}</div>
            <div class="gs-add-item">
                <select id="gs-add-type-${gs.id}">
                    <option value="series">シリーズ</option>
                    <option value="book">書籍</option>
                </select>
                <select id="gs-add-target-${gs.id}">
                    ${allSeries.map((s) => `<option value="series:${s.id}">${escapeHtml(s.name)}</option>`).join("")}
                    ${allBooks.map((b) => `<option value="book:${b.id}">${escapeHtml(b.title)}</option>`).join("")}
                </select>
                <button onclick="addGrandSeriesItem(${gs.id})">追加</button>
            </div>
        </div>`;
    }).join("");
}

async function createGrandSeries() {
    const input = document.getElementById("new-grand-series-name");
    const name = input.value.trim();
    if (!name) return;

    try {
        const res = await fetch("/api/grand-series", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ name }),
        });
        if (res.ok) {
            input.value = "";
            await loadGrandSeries();
            renderGrandSeriesManager();
            loadBooks();
        }
    } catch {}
}

async function startRenameGrandSeries(id, oldName) {
    const el = document.getElementById(`gs-item-${id}`);
    if (!el) return;
    const nameEl = el.querySelector(".gs-list-name");
    const actionsEl = el.querySelector(".series-list-actions");

    const input = document.createElement("input");
    input.className = "inline-edit-input";
    input.value = oldName;

    const save = async () => {
        const newName = input.value.trim();
        if (newName && newName !== oldName) {
            await fetch(`/api/grand-series/${id}`, {
                method: "PUT",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ name: newName }),
            });
        }
        await loadGrandSeries();
        renderGrandSeriesManager();
        loadBooks();
    };

    const cancel = () => {
        nameEl.textContent = oldName;
        actionsEl.style.display = "";
    };

    input.addEventListener("keydown", (e) => {
        if (e.key === "Enter") save();
        if (e.key === "Escape") cancel();
    });
    input.addEventListener("blur", save);

    nameEl.textContent = "";
    nameEl.appendChild(input);
    actionsEl.style.display = "none";
    input.focus();
    input.select();
}

async function deleteGrandSeries(id) {
    const gs = allGrandSeries.find((x) => x.id === id);
    if (!gs) return;
    if (!confirm(`大シリーズ「${gs.name}」を削除しますか？`)) return;

    try {
        const res = await fetch(`/api/grand-series/${id}`, { method: "DELETE" });
        if (res.ok) {
            await loadGrandSeries();
            renderGrandSeriesManager();
            loadBooks();
        }
    } catch {}
}

async function addGrandSeriesItem(gsId) {
    const targetSel = document.getElementById(`gs-add-target-${gsId}`);
    if (!targetSel) return;
    const val = targetSel.value;
    const colonIdx = val.indexOf(":");
    if (colonIdx < 0) return;
    const itemType = val.substring(0, colonIdx);
    const itemId = parseInt(val.substring(colonIdx + 1), 10);

    try {
        await fetch(`/api/grand-series/${gsId}/items`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ item_type: itemType, item_id: itemId }),
        });
        await loadGrandSeries();
        renderGrandSeriesManager();
        loadBooks();
    } catch {}
}

async function removeGrandSeriesItem(gsId, itemType, itemId) {
    try {
        await fetch(`/api/grand-series/${gsId}/items/${itemType}/${itemId}`, { method: "DELETE" });
        await loadGrandSeries();
        renderGrandSeriesManager();
        loadBooks();
    } catch {}
}
