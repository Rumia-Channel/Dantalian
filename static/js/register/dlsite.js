const dlsiteInput = document.getElementById("dlsite-input");
const dlsiteType = document.getElementById("dlsite-media-type");
const dlsiteButton = document.getElementById("dlsite-btn");
let dlsiteSubmitting = false;

function updateDlsiteButton() {
    dlsiteButton.disabled = dlsiteSubmitting || !dlsiteInput.value.trim() || !dlsiteType.value;
}
dlsiteInput.addEventListener("input", updateDlsiteButton);
dlsiteType.addEventListener("change", updateDlsiteButton);
updateDlsiteButton();

document.getElementById("dlsite-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const dlsiteId = dlsiteInput.value.trim();
    const mediaType = dlsiteType.value;
    if (dlsiteSubmitting || !dlsiteId || !mediaType) return;
    const status = document.getElementById("dlsite-status");
    dlsiteSubmitting = true;
    updateDlsiteButton();
    const stopProgress = startRegisterProgress(status, "DLsite から作品情報・表紙・トラック情報を取得 → 保存");
    try {
        const response = await fetch("/api/dlsite", {
            method: "POST", headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ dlsite_id: dlsiteId, media_type: mediaType }),
        });
        const result = await response.json();
        if (!response.ok) {
            status.textContent = result.error || "DLsite から登録できませんでした";
            status.className = "error";
            return;
        }
        const item = result.book || result.cd;
        const idLabel = mediaType === "book" ? "書籍ID" : "音声作品ID";
        status.textContent = `「${item.title}」を${result.source === "cache" ? "登録済みデータ" : "DLsite"}から登録しました（${idLabel}: ${item.id}）`;
        status.className = "success";
        dlsiteInput.value = "";
    } catch {
        status.textContent = "DLsite 登録中に通信エラーが発生しました";
        status.className = "error";
    } finally {
        stopProgress();
        dlsiteSubmitting = false;
        updateDlsiteButton();
    }
});
