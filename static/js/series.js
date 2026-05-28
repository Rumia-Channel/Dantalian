function switchSeriesTab(tab) {
    document.getElementById("tab-series").classList.toggle("active", tab === "series");
    document.getElementById("tab-grand-series").classList.toggle("active", tab === "grand-series");
    document.getElementById("tab-borrowers").classList.toggle("active", tab === "borrowers");
    const tabSettings = document.getElementById("tab-settings");
    if (tabSettings) tabSettings.classList.toggle("active", tab === "settings");
    document.getElementById("panel-series").classList.toggle("hidden", tab !== "series");
    document.getElementById("panel-grand-series").classList.toggle("hidden", tab !== "grand-series");
    document.getElementById("panel-borrowers").classList.toggle("hidden", tab !== "borrowers");
    const panelSettings = document.getElementById("panel-settings");
    if (panelSettings) panelSettings.classList.toggle("hidden", tab !== "settings");
    if (tab === "grand-series") renderGrandSeriesManager();
    if (tab === "borrowers") renderBorrowerList();
    if (tab === "settings" && typeof renderSettingsForm === "function") renderSettingsForm();
}

function renderSeriesManager() {
    const list = document.getElementById("series-list");
    if (allSeries.length === 0) {
        list.innerHTML = '<p class="series-empty">シリーズがありません</p>';
        return;
    }
    list.innerHTML = allSeries.map((s) => {
        const gs = findSeriesGrandSeries(s.id);
        const gsLabel = gs ? ` <span class="series-belong-grand">→ ${escapeHtml(gs.name)}</span>` : "";
        return `
        <div class="series-list-item" id="series-item-${s.id}">
            <span class="series-list-name" ondblclick="startRenameSeries(${s.id}, '${escapeAttr(s.name)}')">${escapeHtml(s.name)}${gsLabel}</span>
            <div class="series-list-actions">
                <button class="btn btn-xs btn-outline-success" onclick="startRenameSeries(${s.id}, '${escapeAttr(s.name)}')">改名</button>
                <button class="btn btn-xs btn-outline-danger" onclick="deleteSeries(${s.id})">削除</button>
            </div>
        </div>`;
    }).join("");
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
    if (!await showConfirm({ message: `シリーズ「${s.name}」を削除しますか？\n所属している本はシリーズから外れます。`, okLabel: "削除" })) return;

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
        list.innerHTML = '<p class="series-empty">大シリーズがありません</p>';
        return;
    }
    list.innerHTML = allGrandSeries.map((gs) => {
        const itemsHtml = gs.items.map((it) => {
            const typeLabel = it.item_type === "series" ? "シリーズ" : it.item_type === "cd" ? "CD" : "書籍";
            return `<div class="gs-item">
                <span class="gs-item-type">${typeLabel}</span>
                <span class="gs-item-name">${escapeHtml(it.name)}</span>
                <button class="gs-item-remove" onclick="removeGrandSeriesItem(${gs.id}, '${it.item_type}', ${it.item_id})" aria-label="項目を削除">
                    <span class="material-icons" aria-hidden="true">close</span>
                </button>
            </div>`;
        }).join("");

        return `
        <div class="gs-list-item" id="gs-item-${gs.id}">
            <div class="gs-list-header">
                <span class="gs-list-name" ondblclick="startRenameGrandSeries(${gs.id}, '${escapeAttr(gs.name)}')">${escapeHtml(gs.name)}</span>
                <div class="series-list-actions">
                    <button class="btn btn-xs btn-outline-success" onclick="startRenameGrandSeries(${gs.id}, '${escapeAttr(gs.name)}')">改名</button>
                    <button class="btn btn-xs btn-outline-danger" onclick="deleteGrandSeries(${gs.id})">削除</button>
                </div>
            </div>
            <div class="gs-items">${itemsHtml}</div>
            <div class="gs-add-item" id="gs-add-item-${gs.id}">
                <div id="gs-add-type-${gs.id}"></div>
                <div id="gs-add-target-${gs.id}"></div>
                <button class="btn btn-xs btn-outline-success" onclick="addGrandSeriesItem(${gs.id})">追加</button>
            </div>
        </div>`;
    }).join("");

    allGrandSeries.forEach((gs) => {
        const typeContainer = document.getElementById(`gs-add-type-${gs.id}`);
        const targetContainer = document.getElementById(`gs-add-target-${gs.id}`);
        if (!typeContainer || !targetContainer) return;

        const targetOpts = allSeries.map((s) => ({ value: `series:${s.id}`, label: s.name }));
        const targetSs = createSearchableSelect(targetContainer, {
            options: targetOpts,
            value: null,
            placeholder: "選択...",
            clearable: false,
        });
        targetContainer._ssInstance = targetSs;

        createSearchableSelect(typeContainer, {
            options: [
                { value: "series", label: "シリーズ" },
                { value: "book", label: "書籍" },
                { value: "cd", label: "CD" },
            ],
            value: "series",
            native: true,
            onChange: (val) => {
                const type = val || "series";
                let opts;
                if (type === "series") {
                    opts = allSeries.map((s) => ({ value: `series:${s.id}`, label: s.name }));
                } else if (type === "cd") {
                    opts = (allCds || []).map((c) => ({ value: `cd:${c.id}`, label: c.title }));
                } else {
                    opts = allBooks.filter((b) => !isBookInGrandSeriesViaSeries(b.id)).map((b) => ({ value: `book:${b.id}`, label: b.title }));
                }
                targetSs.updateOptions(opts);
                targetSs.setValue(null);
            },
        });
    });
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
    if (!await showConfirm({ message: `大シリーズ「${gs.name}」を削除しますか？`, okLabel: "削除" })) return;

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
    const targetContainer = document.getElementById(`gs-add-target-${gsId}`);
    if (!targetContainer) return;
    const ss = targetContainer._ssInstance;
    if (!ss) return;
    const val = String(ss.getValue());
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
