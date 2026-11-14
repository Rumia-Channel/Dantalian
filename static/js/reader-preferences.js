// PDF・画像ZIPの初期読み方向。作品ごとの変更は Bibi が別途保存する。
(() => {
    const STORAGE_KEY = "dantalian_reader_direction";

    function direction() {
        try {
            return localStorage.getItem(STORAGE_KEY) === "ltr" ? "ltr" : "rtl";
        } catch {
            return "rtl";
        }
    }

    function syncControls() {
        const selected = direction();
        document.querySelectorAll("[data-reader-direction]").forEach((button) => {
            const active = button.dataset.readerDirection === selected;
            button.classList.toggle("active", active);
            button.setAttribute("aria-pressed", String(active));
        });
    }

    function initControls() {
        const controls = document.getElementById("reader-direction-controls");
        if (!controls) return;
        const status = document.getElementById("reader-direction-status");
        controls.addEventListener("click", (event) => {
            const button = event.target.closest("[data-reader-direction]");
            if (!button) return;
            try {
                localStorage.setItem(STORAGE_KEY, button.dataset.readerDirection);
                syncControls();
                status.className = "settings-msg settings-msg--ok";
                status.textContent = "読み方向のデフォルトを保存しました。次に開くPDF・画像ZIPから適用します。";
            } catch {
                status.className = "settings-msg settings-msg--error";
                status.textContent = "保存できませんでした。ブラウザのストレージ設定を確認してください。";
            }
        });
        syncControls();
    }

    window.addEventListener("storage", (event) => {
        if (event.key === STORAGE_KEY || event.key === null) syncControls();
    });
    if (document.readyState === "loading") {
        document.addEventListener("DOMContentLoaded", initControls, { once: true });
    } else {
        initControls();
    }

    window.dantalianReaderPreferences = { get direction() { return direction(); } };
})();
