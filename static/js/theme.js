// テーマ（ライト / ダーク）の制御。
// localStorage の保存値: "system" | "light" | "dark"（既定 "system"）。
// <head> で同期的に読み込まれ、最初の描画前に data-theme を確定するため
// テーマのチラつきは起きない。CSS は html[data-theme] のみを見ればよい。
(() => {
    const STORAGE_KEY = "dantalian_theme";
    const MODES = ["system", "light", "dark"];
    const FALLBACK_META = { dark: "#05070f", light: "#eff1f7" };
    const media = window.matchMedia ? window.matchMedia("(prefers-color-scheme: dark)") : null;


    function savedMode() {
        const mode = localStorage.getItem(STORAGE_KEY);
        return MODES.includes(mode) ? mode : "system";
    }

    function resolved(mode) {
        if (mode === "light" || mode === "dark") return mode;
        return media && media.matches ? "dark" : "light";
    }

    function syncMetaTheme(resolvedTheme) {
        const meta = document.querySelector('meta[name="theme-color"]');
        if (!meta) return;
        // 描画済みなら実測値を使い、未描画でもフォールバック値で更新する。
        let color = null;
        try {
            const bg = getComputedStyle(document.documentElement).getPropertyValue("--color-bg-body").trim();
            if (bg) color = bg;
        } catch {}
        meta.setAttribute("content", color || FALLBACK_META[resolvedTheme] || FALLBACK_META.dark);
    }

    function applyTheme(mode) {
        const theme = resolved(mode);
        document.documentElement.dataset.theme = theme;
        document.documentElement.style.colorScheme = theme;
        syncMetaTheme(theme);
        syncControls(mode, theme);
        return theme;
    }

    function setTheme(mode) {
        if (!MODES.includes(mode)) return;
        localStorage.setItem(STORAGE_KEY, mode);
        applyTheme(mode);
    }

    function syncControls(mode, theme) {
        document.querySelectorAll(".theme-btn[data-theme-mode]").forEach((btn) => {
            const active = btn.dataset.themeMode === mode;
            btn.classList.toggle("active", active);
            btn.setAttribute("aria-pressed", String(active));
        });
    }


    function initControls() {
        document.addEventListener("click", (event) => {
            const btn = event.target.closest(".theme-btn[data-theme-mode]");
            if (btn) setTheme(btn.dataset.themeMode);
        });
        syncControls(savedMode(), resolved(savedMode()));
    }

    if (media) {
        media.addEventListener("change", () => {
            if (savedMode() === "system") applyTheme("system");
        });
    }

    applyTheme(savedMode());
    if (document.readyState === "loading") {
        document.addEventListener("DOMContentLoaded", initControls);
    } else {
        initControls();
    }

    window.dantalianTheme = {
        get mode() { return savedMode(); },
        get resolved() { return resolved(savedMode()); },
        set: setTheme,
        apply: applyTheme,
    };
})();
