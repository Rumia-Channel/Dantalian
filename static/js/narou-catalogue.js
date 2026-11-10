// narou.rs is authoritative. Local book/file rows cache its current tagged shelf
// for the existing reader; every visible bookshelf refresh checks the remote API.
async function refreshNarouCatalogue() {
    const status = document.getElementById("narou-sync-status");
    status.hidden = false;
    status.textContent = "Web 小説を narou.rs と同期中…";
    try {
        const response = await fetch("/api/narou/sync", { method: "POST", cache: "no-store" });
        const report = await response.json();
        if (!response.ok) throw new Error(report.error || `HTTP ${response.status}`);
        if (report.failed.length) {
            status.textContent = `Web 小説の一部を更新できませんでした（${report.failed.length} 件）。${report.failed.map(failure => `${failure.title}：${failure.error}`).join(" / ")}`;
        } else if (report.tags.length) {
            status.textContent = `Web 小説 ${report.matched} 件を同期済み（${new Date().toLocaleTimeString()}）。表示中は 5 分ごとに更新します。`;
        } else {
            status.textContent = "";
            status.hidden = true;
        }
    } catch (error) {
        status.textContent = `Web 小説の同期に失敗しました。前回の同期内容を表示します。${error.message}`;
    }
}

setInterval(() => {
    if (document.visibilityState === "visible") void refreshHome();
}, 5 * 60 * 1000);

document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") void refreshHome();
});
