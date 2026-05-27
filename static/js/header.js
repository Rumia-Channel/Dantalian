(function initHeader() {
    var app = document.getElementById("app");
    if (!app) return;
    var active = document.body.dataset.activeNav || "home";

    var header = document.createElement("div");
    header.id = "tsukuyomi-header";
    header.innerHTML =
        '<h1>Tsukuyomi</h1>' +
        '<nav>' +
            '<a href="/" id="nav-home">書籍一覧</a>' +
            '<a href="/register/" id="nav-register">登録</a>' +
            '<a href="/manage/" id="nav-manage">シリーズ</a>' +
            '<a href="/authors/" id="nav-authors">著者</a>' +
            '<a href="/edit/" id="nav-edit">編集</a>' +
        '</nav>' +
        '<div class="settings-bar">' +
            '<span class="settings-label">表示幅</span>' +
            '<div class="settings-buttons" id="width-buttons"></div>' +
        '</div>';

    app.insertBefore(header, app.firstChild);

    var navEl = document.getElementById("nav-" + active);
    if (navEl) navEl.classList.add("active");
})();
