const authorsContent = document.getElementById("authors-content");
let authors = [];

const params = new URLSearchParams(window.location.search);
const editId = params.get("edit");

(async () => {
    await loadAuthors();
    if (editId) {
        renderAuthorEdit(parseInt(editId, 10));
    } else {
        renderAuthorList();
    }
})();

async function loadAuthors() {
    try {
        const res = await fetch("/api/authors");
        authors = await res.json();
    } catch {
        authors = [];
    }
}

function renderAuthorList() {
    window.history.replaceState(null, "", "/authors/");
    authorsContent.innerHTML = `
        <h2>アーティスト一覧 <span style="font-size:0.8rem;color:#888;">(${authors.length}件)</span></h2>
        <div class="author-create">
            <input type="text" id="new-author-name" placeholder="アーティスト名">
            <input type="text" id="new-author-transcription" placeholder="ヨミガナ（任意）">
            <input type="text" id="new-author-ndl-id" placeholder="NDL ID（任意）">
            <button onclick="createAuthor()" class="btn btn-primary">追加</button>
        </div>
        <div class="edit-author-list">
            ${authors.map((a) => `
                <div class="edit-author-item">
                    <div class="edit-author-info">
                        <div class="edit-author-name">${escapeHtml(a.name)}</div>
                        <div class="edit-author-meta">
                            ${a.ndl_id ? `<span>NDL: ${escapeHtml(a.ndl_id)}</span>` : ""}
                            ${a.transcription ? `<span>${escapeHtml(a.transcription)}</span>` : ""}
                        </div>
                    </div>
                    <button class="btn btn-xs btn-outline-success" onclick="renderAuthorEdit(${a.id})">編集</button>
                </div>
            `).join("")}
            ${authors.length === 0 ? '<p class="series-empty">アーティストがいません</p>' : ""}
        </div>
    `;
}

function renderAuthorEdit(id) {
    const author = authors.find((a) => a.id === id);
    if (!author) {
        renderAuthorList();
        return;
    }

    window.history.replaceState(null, "", `/authors/?edit=${author.id}`);
    authorsContent.innerHTML = `
        <h2>アーティスト情報編集</h2>
        <div class="edit-author-edit-header">
            <span class="edit-author-id">ID: ${author.id}</span>
            ${author.ndl_id ? `<span class="edit-author-ndl">NDL: ${escapeHtml(author.ndl_id)}</span>` : ""}
        </div>
        <form class="edit-form" onsubmit="saveAuthor(event, ${author.id})">
            <div class="edit-field">
                <label>名前 <span class="edit-required">*</span></label>
                <input type="text" name="name" value="${escapeAttr(author.name)}" required>
            </div>
            <div class="edit-field">
                <label>ヨミガナ</label>
                <input type="text" name="transcription" value="${escapeAttr(author.transcription || '')}">
            </div>
            <div class="edit-field">
                <label>NDL ID</label>
                <input type="text" name="ndl_id" value="${escapeAttr(author.ndl_id || '')}">
            </div>
        <div class="edit-actions">
                <button type="button" class="btn btn-md btn-ghost" onclick="renderAuthorList()">一覧に戻る</button>
                <button type="button" class="btn btn-md btn-outline-danger" onclick="deleteAuthor(${author.id})">削除</button>
                <button type="submit" class="btn btn-md btn-primary">保存</button>
            </div>
        </form>
        <h3>重複の統合</h3>
        <p>同じ人物の別表記にチェックを入れて統合すると、書籍・CD・曲の関連付けがこの ID:${author.id} にまとまります。元に戻せません。</p>
        <div class="edit-author-list">
            ${authors.filter((a) => a.id !== author.id).map((a) => `
                <label class="edit-author-item">
                    <input type="checkbox" class="merge-dupe" value="${a.id}">
                    <span class="edit-author-info">
                        <span class="edit-author-name">${escapeHtml(a.name)}</span>
                        <span class="edit-author-meta">ID: ${a.id}${a.ndl_id ? ` NDL: ${escapeHtml(a.ndl_id)}` : ""}</span>
                    </span>
                </label>`).join("")}
        </div>
        <div class="edit-actions">
            <button type="button" class="btn btn-md btn-primary" onclick="mergeAuthors(${author.id})">選択をこのアーティストに統合</button>
        </div>
    `;
}

async function createAuthor() {
    const nameEl = document.getElementById("new-author-name");
    const transEl = document.getElementById("new-author-transcription");
    const ndlEl = document.getElementById("new-author-ndl-id");
    const name = nameEl.value.trim();
    if (!name) return;

    try {
        const res = await fetch("/api/authors", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
                name,
                transcription: transEl.value.trim() || null,
                ndl_id: ndlEl.value.trim() || null,
            }),
        });
        if (res.ok) {
            await loadAuthors();
            renderAuthorList();
        }
    } catch {}
}

async function saveAuthor(e, authorId) {
    e.preventDefault();
    const fd = new FormData(e.target);
    const body = {};
    for (const [key, val] of fd.entries()) {
        body[key] = val === "" ? null : val;
    }
    try {
        const res = await fetch(`/api/authors/${authorId}`, {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
        });
        if (res.ok) {
            await loadAuthors();
            renderAuthorEdit(authorId);
        }
    } catch {}
}

async function deleteAuthor(authorId) {
    const author = authors.find((item) => item.id === authorId);
    if (!author) return;
    const ok = await showConfirm({
        message: `アーティスト「${author.name}」を削除しますか？\n書籍・CD・曲との関連付けも解除されます。`,
        okLabel: "削除",
    });
    if (!ok) return;

    try {
        const res = await fetch(`/api/authors/${authorId}`, { method: "DELETE" });
        if (res.ok) {
            await loadAuthors();
            renderAuthorList();
        } else {
            const body = await res.json().catch(() => ({}));
            alert(`アーティストの削除に失敗しました (HTTP ${res.status})${body.error ? `: ${body.error}` : ""}`);
        }
    } catch {
        alert("アーティストの削除中に通信エラーが発生しました");
    }
}

async function mergeAuthors(survivorId) {
    const survivor = authors.find((item) => item.id === survivorId);
    if (!survivor) return;
    const duplicateIds = [...document.querySelectorAll(".merge-dupe:checked")]
        .map((el) => parseInt(el.value, 10))
        .filter((id) => Number.isInteger(id) && id !== survivorId);
    if (duplicateIds.length === 0) return;
    const ok = await showConfirm({
        message: `「${survivor.name}」(ID:${survivorId})に${duplicateIds.length}件を統合しますか？\n書籍・CD・曲の関連付けが付け替わり、統合されたアーティストは削除されます。元に戻せません。`,
        okLabel: "統合",
    });
    if (!ok) return;

    try {
        const res = await fetch("/api/authors/merge", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ survivor_id: survivorId, duplicate_ids: duplicateIds }),
        });
        if (res.ok) {
            await loadAuthors();
            renderAuthorEdit(survivorId);
        } else {
            const body = await res.json().catch(() => ({}));
            alert(`アーティストの統合に失敗しました (HTTP ${res.status})${body.error ? `: ${body.error}` : ""}`);
        }
    } catch {
        alert("アーティストの統合中に通信エラーが発生しました");
    }
}
