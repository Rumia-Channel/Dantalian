// Web 小説 (narou.rs) のカタログはブラウザが narou.rs から直接取得する。
//
// Dantalian には写しや EPUB を作らない。ローカル書籍の一覧とは独立して読み込む:
// 取得が遅くても失敗しても、ローカル書籍はそのまま表示し、異常は状態行で知らせる。
const NAROU_CATALOGUE_MIN_INTERVAL_MS = 60 * 1000;
let narouCatalogueLoadedAt = 0;

async function refreshNarouCatalogue() {
    const status = document.getElementById("narou-catalogue-status");
    const client = window.narouClient;
    if (!client) return;
    setNarouCatalogueStatus(status, "Web 小説を narou.rs から取得中…");
    try {
        const result = await client.loadCatalogueOnce();
        narouCatalogueLoadedAt = Date.now();
        if (result.state === "disabled") {
            setNarouCatalogueStatus(status, "");
        } else if (!result.items.length) {
            setNarouCatalogueStatus(status, "対象の Web 小説はありません。");
        } else {
            const tags = result.tags.join("、");
            setNarouCatalogueStatus(status,
                `Web 小説 ${result.items.length} 件を narou.rs から直接取得しました（対象タグ: ${tags}）。カバーは表示されません。`);
        }
        // 届いたぶんだけ一覧を描き直す（ローカル書籍の読み込みを妨げない）。
        document.dispatchEvent(new CustomEvent("dantalian:narou-catalogue", { detail: result }));
    } catch (error) {
        client.items = [];
        client.state = "error";
        document.dispatchEvent(new CustomEvent("dantalian:narou-catalogue"));
        const source = client.openUrl(error.connection);
        setNarouCatalogueStatus(status,
            `Web 小説を取得できませんでした: ${error.message}。ローカル書籍はそのまま表示しています。`,
            source ? { href: source, label: "narou.rs でログイン" } : null);
    }
}

function setNarouCatalogueStatus(status, message, link) {
    if (!status) return;
    status.textContent = message || "";
    if (link && link.href) {
        const anchor = document.createElement("a");
        anchor.href = link.href;
        anchor.target = "_blank";
        anchor.rel = "noopener noreferrer";
        anchor.textContent = link.label || "narou.rs を開く";
        status.append(" ", anchor);
    }
    status.hidden = !message;
}

setInterval(() => {
    if (document.visibilityState === "visible") void refreshNarouCatalogue();
}, 5 * 60 * 1000);

document.addEventListener("visibilitychange", () => {
    // タブ back 回来た直後は取り直さない（5 分ごとの更新に任せる）。
    if (document.visibilityState !== "visible") return;
    if (Date.now() - narouCatalogueLoadedAt < NAROU_CATALOGUE_MIN_INTERVAL_MS) return;
    void refreshNarouCatalogue();
});
