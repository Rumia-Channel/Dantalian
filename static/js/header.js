(function initHeader() {
    var app = document.getElementById("app");
    if (!app) return;
    var active = document.body.dataset.activeNav || "home";

    var header = document.createElement("div");
    header.id = "dantalian-header";
    header.innerHTML =
        '<h1 class="header-brand"><a href="/">Dantalian</a></h1>' +
        '<nav>' +
            '<a href="/" id="nav-home">一覧</a>' +
            '<a href="/books/" id="nav-books"><span class="nav-book-label">本</span></a>' +
            '<a href="/music/" id="nav-music">音楽</a>' +
            '<a href="/register/" id="nav-register">登録</a>' +
            '<a href="/manage/" id="nav-manage">管理</a>' +
            '<a href="/authors/" id="nav-authors">アーティスト</a>' +
            '<a href="/licenses/" id="nav-licenses">ライセンス</a>' +
        '</nav>' +
        '<div class="header-auth hidden" id="auth-settings">' +
            '<button type="button" class="btn btn-ghost btn-sm auth-open" id="auth-open">APIログイン</button>' +
        '</div>' +
        '<div class="settings-bar">' +
            '<span class="settings-label">表示幅</span>' +
            '<div class="settings-buttons" id="width-buttons"></div>' +
        '</div>';

    app.insertBefore(header, app.firstChild);

    var navEl = document.getElementById("nav-" + active);
    if (navEl) {
        navEl.classList.add("active");
        navEl.setAttribute("aria-current", "page");
    }

    var credit = document.createElement("footer");
    credit.className = "service-credit";
    credit.innerHTML =
        '書誌情報の取得に<a href="https://ndlsearch.ndl.go.jp/help/api" target="_blank" rel="noopener noreferrer">国立国会図書館サーチのAPI</a>・<a href="https://openlibrary.org/developers/api" target="_blank" rel="noopener noreferrer">Open LibraryのAPI</a>、' +
        '著者典拠の取得に<a href="https://id.ndl.go.jp/information/sparql-11/" target="_blank" rel="noopener noreferrer">Web NDL AuthoritiesのAPI</a>を利用しています。';
    app.appendChild(credit);

})();
