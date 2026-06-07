const WIDTH_PRESETS = [
    { label: "狭", pct: 50 },
    { label: "標準", pct: 70 },
    { label: "広", pct: 85 },
    { label: "全幅", pct: 100 },
];

function applyContentWidth(pct) {
    document.documentElement.style.setProperty("--content-width", pct + "%");
    localStorage.setItem("dantalian_content_width", pct);
    document.querySelectorAll(".width-btn").forEach((btn) => {
        btn.classList.toggle("active", parseInt(btn.dataset.pct, 10) === pct);
    });
}

(function initContentWidth() {
    const container = document.getElementById("width-buttons");
    if (!container) {
        const saved = localStorage.getItem("dantalian_content_width");
        if (saved) document.documentElement.style.setProperty("--content-width", saved + "%");
        return;
    }
    const saved = localStorage.getItem("dantalian_content_width");
    const defaultPct = saved ? parseInt(saved, 10) : 70;

    container.innerHTML = WIDTH_PRESETS.map((p) =>
        `<button class="width-btn" data-pct="${p.pct}">${p.label}</button>`
    ).join("");

    container.addEventListener("click", (e) => {
        const btn = e.target.closest(".width-btn");
        if (!btn) return;
        applyContentWidth(parseInt(btn.dataset.pct, 10));
    });

    applyContentWidth(defaultPct);
})();
