// --- Audiobook registration ---
const audiobookForm = document.getElementById("audiobook-form");
const audiobookInput = document.getElementById("audiobook-input");
const audiobookJanInput = document.getElementById("audiobook-jan-input");
const audiobookBtn = document.getElementById("audiobook-btn");
const audiobookStatus = document.getElementById("audiobook-status");

audiobookInput.addEventListener("input", () => {
    audiobookInput.value = audiobookInput.value
        .replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xFEE0))
        .replace(/[\s\u3000\-－ー]/g, "");
});

if (audiobookJanInput) {
    audiobookJanInput.addEventListener("input", () => {
        audiobookJanInput.value = audiobookJanInput.value
            .replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xFEE0))
            .replace(/[\s\u3000\-－ー]/g, "");
    });
}

audiobookForm.addEventListener("submit", async (e) => {
    e.preventDefault();
    const isbn = audiobookInput.value.trim().replace(/-/g, "");
    const jan = audiobookJanInput ? audiobookJanInput.value.trim().replace(/-/g, "") : "";
    if (!isbn && !jan) return;

    const parentBookId = document.getElementById("audiobook-parent-book-id");
    const parentId = parentBookId ? parseInt(parentBookId.value) || null : null;

    audiobookBtn.disabled = true;
    audiobookStatus.textContent = "検索中...";
    audiobookStatus.className = "";

    try {
        const code = jan || isbn;
        const body = { jan: code, media_type: "audiobook" };
        if (parentId) body.parent_book_id = parentId;
        const res = await fetch("/api/cds", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
        });
        const data = await res.json();

        if (!res.ok) {
            audiobookStatus.textContent = data.error || "登録に失敗しました";
            audiobookStatus.className = "error";
            return;
        }

        let sourceLabel = "MusicBrainz";
        audiobookStatus.textContent = `「${data.title}」を${sourceLabel}からオーディオブックとして登録しました`;
        audiobookStatus.className = "success";
        audiobookInput.value = "";
        if (audiobookJanInput) audiobookJanInput.value = "";
    } catch (err) {
        audiobookStatus.textContent = "通信エラーが発生しました";
        audiobookStatus.className = "error";
    } finally {
        audiobookBtn.disabled = false;
    }
});
