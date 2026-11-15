// カタログ系ページ（一覧 / 本 / 音楽）の設定バー項目をここで注入する。
// 表示するページ: カードサイズはカードグリッドのあるページのみ、
// 並び順はページごとのソートボタン、種類フィルタは本+音楽の混在カタログのみ。
const CARD_SIZE_KEY = "dantalian_card_size";
const CARD_SIZES = ["lg", "md", "sm"];

function applyCardSize(size) {
    if (!CARD_SIZES.includes(size)) return;
    localStorage.setItem(CARD_SIZE_KEY, size);
    document.documentElement.dataset.cardSize = size;
    syncCardSizeButtons();
}

function syncCardSizeButtons() {
    const current = document.documentElement.dataset.cardSize || "md";
    document.querySelectorAll("#card-size-buttons .width-btn").forEach((b) => {
        const active = b.dataset.cardSize === current;
        b.classList.toggle("active", active);
        b.setAttribute("aria-pressed", String(active));
    });
}

(function initCatalogueControls() {
    const bar = document.querySelector(".settings-bar");
    if (!bar) return;

    const isBooksCatalogue = document.body.dataset.catalogue === "books";
    const hasBookGrid = Boolean(document.getElementById("book-grid"));
    const hasMusicGrid = Boolean(document.getElementById("music-grid"));

    // カードサイズ（小/中/大）。中は音楽グリッドの現在の密度が基準。
    if (hasBookGrid || hasMusicGrid) {
        const sizeGroup = document.createElement("div");
        sizeGroup.className = "settings-group";
        sizeGroup.innerHTML =
            '<span class="settings-label">カード</span>' +
            '<div class="settings-buttons" id="card-size-buttons">' +
            '<button class="width-btn" data-card-size="sm">小</button>' +
            '<button class="width-btn" data-card-size="md">中</button>' +
            '<button class="width-btn" data-card-size="lg">大</button>' +
            '</div>';
        bar.appendChild(sizeGroup);
        sizeGroup.addEventListener("click", (e) => {
            const btn = e.target.closest(".width-btn");
            if (btn) applyCardSize(btn.dataset.cardSize);
        });
        syncCardSizeButtons();
    }

    if (hasBookGrid) {
        const sortGroup = document.createElement("div");
        sortGroup.className = "settings-group";
        sortGroup.innerHTML =
            '<span class="settings-label">並び順</span>' +
            '<div class="settings-buttons" id="sort-buttons">' +
            '<button class="width-btn" data-sort="id">登録順</button>' +
            '<button class="width-btn" data-sort="title">タイトル順</button>' +
            '<button class="width-btn" data-sort="publish_date">発売日順</button>' +
            '</div>';
        bar.appendChild(sortGroup);
        if (!isBooksCatalogue) {
            const typeGroup = document.createElement("div");
            typeGroup.className = "settings-group";
            typeGroup.innerHTML =
                '<span class="settings-label">表示</span>' +
                '<div class="settings-buttons" id="type-filter-buttons">' +
                '<button class="width-btn active" data-type="all">全</button>' +
                '<button class="width-btn" data-type="book">書籍</button>' +
                '<button class="width-btn" data-type="cd">CD</button>' +
                '<button class="width-btn" data-type="audiobook">AB</button>' +
                '</div>';
            bar.appendChild(typeGroup);
        }
        return;
    }

    if (hasMusicGrid) {
        const sortGroup = document.createElement("div");
        sortGroup.className = "settings-group";
        sortGroup.innerHTML =
            '<span class="settings-label">並び順</span>' +
            '<div class="settings-buttons" id="music-sort-buttons">' +
            '<button class="width-btn" data-sort="id">登録順</button>' +
            '<button class="width-btn" data-sort="title">タイトル順</button>' +
            '<button class="width-btn" data-sort="publish_date">発売日順</button>' +
            '</div>';
        bar.appendChild(sortGroup);
    }
})();
