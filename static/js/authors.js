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
