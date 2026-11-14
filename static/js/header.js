(function initHeader() {
    var app = document.getElementById("app");
    if (!app) return;
    var active = document.body.dataset.activeNav || "home";

    var header = document.createElement("div");
    header.id = "dantalian-header";
    header.innerHTML =
        '<h1 class="header-brand"><a href="/"><span class="header-brand-initial">D</span>antalian</a></h1>' +
        '<div class="header-tools">' +
            '<div class="header-auth hidden" id="auth-settings">' +
                '<button type="button" class="btn btn-ghost btn-sm auth-open" id="auth-open">APIログイン</button>' +
            '</div>' +
        '</div>' +
        '<nav>' +
            '<a href="/" id="nav-home"><span class="nav-icon material-icons" aria-hidden="true">home</span><span class="nav-label">一覧</span></a>' +
            '<a href="/books/" id="nav-books"><span class="nav-icon material-icons" aria-hidden="true">menu_book</span><span class="nav-label nav-book-label">本</span></a>' +
            '<a href="/music/" id="nav-music"><span class="nav-icon material-icons" aria-hidden="true">music_note</span><span class="nav-label">音楽</span></a>' +
            '<a href="/register/" id="nav-register"><span class="nav-icon material-icons" aria-hidden="true">add_circle</span><span class="nav-label">登録</span></a>' +
            '<a href="/manage/" id="nav-manage"><span class="nav-icon material-icons" aria-hidden="true">settings</span><span class="nav-label">管理</span></a>' +
            '<a href="/authors/" id="nav-authors"><span class="nav-icon material-icons" aria-hidden="true">person</span><span class="nav-label">アーティスト</span></a>' +
            '<a href="/licenses/" id="nav-licenses"><span class="nav-icon material-icons" aria-hidden="true">gavel</span><span class="nav-label">ライセンス</span></a>' +
        '</nav>' +
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

    /* 短いページでフッターを画面最下部へ押し下げる伸縮スペーサー */
    var spacer = document.createElement("div");
    spacer.className = "app-spacer";
    spacer.setAttribute("aria-hidden", "true");
    app.appendChild(spacer);

    var credit = document.createElement("footer");
    credit.className = "service-credit";
    credit.innerHTML =
        '書誌情報の取得に<a href="https://ndlsearch.ndl.go.jp/help/api" target="_blank" rel="noopener noreferrer">国立国会図書館サーチのAPI</a>・<a href="https://openlibrary.org/developers/api" target="_blank" rel="noopener noreferrer">Open LibraryのAPI</a>、' +
        '著者典拠の取得に<a href="https://id.ndl.go.jp/information/sparql-11/" target="_blank" rel="noopener noreferrer">Web NDL AuthoritiesのAPI</a>を利用しています。';
    app.appendChild(credit);

})();
