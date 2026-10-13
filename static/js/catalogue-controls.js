(function initCatalogueControls() {
    const bar = document.querySelector(".settings-bar");
    if (!bar) return;
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
    if (document.body.dataset.catalogue === "books") return;
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
})();
