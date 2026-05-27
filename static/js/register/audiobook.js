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

    audiobookBtn.disabled = true;
    audiobookStatus.textContent = "検索中...";
    audiobookStatus.className = "";

    try {
        let res;
        if (jan) {
            res = await fetch("/api/books", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ jan, media_type: "audiobook" }),
            });
        } else {
            res = await fetch("/api/books", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ isbn, media_type: "audiobook" }),
            });
        }
        const data = await res.json();

        if (!res.ok) {
            audiobookStatus.textContent = data.error || "登録に失敗しました";
            audiobookStatus.className = "error";
            return;
        }

        let sourceLabel;
        if (data.source === "amazon") sourceLabel = "Amazon";
        else if (data.source === "musicbrainz") sourceLabel = "MusicBrainz";
        else sourceLabel = "国立国会図書館";
        audiobookStatus.textContent = `「${data.book.title}」を${sourceLabel}からオーディオブックとして登録しました`;
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
