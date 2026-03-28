function toggleSeriesManager() {
    const panel = document.getElementById("series-manager");
    panel.classList.toggle("hidden");
    if (!panel.classList.contains("hidden")) {
        renderSeriesManager();
    }
}

function renderSeriesManager() {
    const list = document.getElementById("series-list");
    if (allSeries.length === 0) {
        list.innerHTML = '<p style="color:#555;font-size:0.85rem;padding:0.5rem 0;">シリーズがありません</p>';
        return;
    }
    list.innerHTML = allSeries.map((s) => `
        <div class="series-list-item" id="series-item-${s.id}">
            <span class="series-list-name" ondblclick="startRenameSeries(${s.id}, '${escapeAttr(s.name)}')">${escapeHtml(s.name)}</span>
            <div class="series-list-actions">
                <button class="btn-rename" onclick="startRenameSeries(${s.id}, '${escapeAttr(s.name)}')">改名</button>
                <button class="btn-delete" onclick="deleteSeries(${s.id})">削除</button>
            </div>
        </div>
    `).join("");
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
            renderSeriesManager();
            loadBooks();
        }
    } catch {}
}
