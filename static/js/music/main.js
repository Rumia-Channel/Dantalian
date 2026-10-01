// 音楽ライブラリページ: CD/オーディオブックの一覧表示と、インラインプレイヤーの起動。

const musicGrid = document.getElementById("music-grid");
const musicCount = document.getElementById("music-count");
const playerRoot = document.getElementById("music-player-root");

let allAlbums = [];           // 再生可能トラックを持つ CD 一覧
let currentFilter = "all";

const MUSIC_VIEWS = ["artist", "grid"];
const MUSIC_SORTS = ["id", "title", "publish_date"];

const storedView = localStorage.getItem("dantalian_music_view");
const storedSort = localStorage.getItem("dantalian_music_sort");
let currentView = MUSIC_VIEWS.includes(storedView) ? storedView : "artist";
let currentSort = MUSIC_SORTS.includes(storedSort) ? storedSort : "id";

const player = createPlayerUI(playerRoot);
window.musicPlayer = player;

function loadCollapsedArtistGroups() {
    try {
        const saved = JSON.parse(localStorage.getItem("dantalian_music_collapsed_artist_groups") || "[]");
        return Array.isArray(saved) ? saved : [];
    } catch {
        return [];
    }
}

const collapsedArtistGroups = new Set(loadCollapsedArtistGroups());

function saveCollapsedArtistGroups() {
    localStorage.setItem("dantalian_music_collapsed_artist_groups", JSON.stringify([...collapsedArtistGroups]));
}

function albumMediaType(cd) {
    return cd.media_type === "audiobook" ? "audiobook" : "cd";
}

function playableAlbums() {
    return allAlbums.filter((cd) => (cd.tracks || []).some((t) => t.file_hash));
}

function filteredMedia() {
    const list = playableAlbums();
    const playlists = getPlaylists();
    if (currentFilter === "all") return { albums: list, playlists };
    return { albums: list.filter((cd) => albumMediaType(cd) === currentFilter), playlists: [] };
}

// 並び順: 登録順は新しい順、発売日順は新しい順・未設定は末尾 (一覧ページと同じ)。
function sortAlbums(list) {
    return [...list].sort((a, b) => {
        let cmp = 0;
        if (currentSort === "title") {
            cmp = a.title.localeCompare(b.title, "ja");
        } else if (currentSort === "publish_date") {
            cmp = (b.publish_date || "").localeCompare(a.publish_date || "");
        }
        return cmp !== 0 ? cmp : b.id - a.id;
    });
}

// 一覧ページと同じグループキー: authors 登録アーティストは ID キー、
// タグ由来のアーティスト名は正規化名キーでまとめる。
function getAlbumArtistGroup(cd) {
    const artist = getCdArtistIdentity(cd);
    if (!artist) return null;
    const key = artist.source === "author" && artist.id != null
        ? `author:${artist.id}`
        : `artist:${normalizeSearchText(artist.name)}`;
    return { key, name: artist.name };
}

function renderAlbumCard(cd) {
    const type = albumMediaType(cd);
    const badge = type === "audiobook"
        ? '<span class="music-album-badge music-album-badge--audiobook">AB</span>'
        : '<span class="music-album-badge">CD</span>';
    const cover = cd.cover_url
        ? `<img class="music-album-cover" src="/images/${cd.cover_url}" alt="${escapeAttr(cd.title)}" loading="lazy">`
        : `<div class="music-album-coverfallback"><span class="material-icons">album</span></div>`;
    const trackCount = (cd.tracks || []).filter((t) => t.file_hash).length;
    const artistName = getCdArtistName(cd);
    const artist = artistName ? escapeHtml(artistName) : "&nbsp;";
    return `
    <div class="music-album" data-cd-id="${cd.id}" tabindex="0" role="button" aria-label="${escapeAttr(cd.title)} を表示">
        <div class="music-album-coverwrap">
            ${cover}
            ${badge}
            <div class="music-album-play">
                <button type="button" class="music-album-play-btn" data-album-action="play" aria-label="${escapeAttr(cd.title)}を再生">
                    <span class="material-icons">play_arrow</span>
                </button>
            </div>
        </div>
        <div class="music-album-name">${escapeHtml(cd.title)}</div>
        <div class="music-album-artist">${artist}</div>
        <div class="music-album-meta">${trackCount} 曲</div>
    </div>`;
}

function renderArtistGroup(key, label, count, cardsHtml, isNone) {
    const isCollapsed = collapsedArtistGroups.has(key);
    const headerHtml = isNone
        ? `<button type="button" class="author-group-header author-group-header--none" aria-expanded="${!isCollapsed}">
                <span class="author-group-name">${escapeHtml(label)}</span>
                <span class="author-group-count">${count}件</span>
            </button>`
        : `<button type="button" class="author-group-header" aria-expanded="${!isCollapsed}">
                <span class="author-group-name">${escapeHtml(label)}</span>
                <span class="author-group-count">${count}件</span>
            </button>`;
    return `<div class="author-group${isCollapsed ? " author-group--collapsed" : ""}" data-author-group="${escapeAttr(key)}">${headerHtml}<div class="author-group-grid">${cardsHtml}</div></div>`;
}

// アーティスト別表示: 「プレイリスト」という作者グループを先頭に、
// その後アーティスト名順、未設定は末尾。
function renderAlbumsByArtist(media) {
    let html = "";

    if (media.playlists.length > 0) {
        html += renderArtistGroup(
            "__playlists__",
            "プレイリスト",
            media.playlists.length,
            media.playlists.map(renderPlaylistCard).join(""),
            false
        );
    }

    const groupMap = new Map();
    for (const cd of media.albums) {
        const artist = getAlbumArtistGroup(cd);
        const key = artist ? artist.key : "__none__";
        if (!groupMap.has(key)) {
            groupMap.set(key, { artist, albums: [] });
        }
        groupMap.get(key).albums.push(cd);
    }

    const entries = [...groupMap.entries()].sort((a, b) => {
        if (a[0] === "__none__") return 1;
        if (b[0] === "__none__") return -1;
        return a[1].artist.name.localeCompare(b[1].artist.name, "ja");
    });

    for (const [key, { artist, albums }] of entries) {
        const sorted = sortAlbums(albums);
        html += renderArtistGroup(
            key,
            artist ? artist.name : "アーティスト未設定",
            albums.length,
            sorted.map(renderAlbumCard).join(""),
            !artist
        );
    }

    return html;
}

function renderGrid() {
    const media = filteredMedia();
    const count = media.albums.length + media.playlists.length;
    musicCount.textContent = `(${count}件)`;

    if (count === 0) {
        musicGrid.innerHTML = `
            <div class="music-empty">
                <span class="material-icons">library_music</span>
                再生できるアルバムがありません。<br>
                音声ファイル付きのCDまたはオーディオブックを登録してください。
            </div>`;
        return;
    }

    if (currentView === "artist") {
        musicGrid.innerHTML = renderAlbumsByArtist(media);
    } else {
        const albumHtml = sortAlbums(media.albums).map(renderAlbumCard).join("");
        musicGrid.innerHTML = albumHtml + media.playlists.map(renderPlaylistCard).join("");
    }
}

function openAlbum(cdId, autoplay) {
    player.setAlbums(playableAlbums());
    player.openAlbum(cdId, !!autoplay);
}

// グリッド操作 (クリック / キーボード) — 自動再生しない
musicGrid.addEventListener("click", (e) => {
    const groupHeader = e.target.closest(".author-group-header");
    if (groupHeader && musicGrid.contains(groupHeader)) {
        const group = groupHeader.closest(".author-group");
        const groupKey = group?.dataset.authorGroup;
        if (!group || !groupKey) return;

        e.preventDefault();
        e.stopPropagation();

        const isCollapsed = group.classList.toggle("author-group--collapsed");
        groupHeader.setAttribute("aria-expanded", String(!isCollapsed));

        if (isCollapsed) {
            collapsedArtistGroups.add(groupKey);
        } else {
            collapsedArtistGroups.delete(groupKey);
        }
        saveCollapsedArtistGroups();
        return;
    }

    const card = e.target.closest(".music-album");
    if (!card) return;
    const playlistId = card.dataset.playlistId;
    if (playlistId) {
        if (e.target.closest("[data-playlist-action=\"edit\"]")) {
            e.stopPropagation();
            openPlaylistEditor(playlistId);
            return;
        }
        if (e.target.closest("[data-playlist-action=\"delete\"]")) {
            e.stopPropagation();
            confirmDeletePlaylist(playlistId).catch((err) => console.error("deletePlaylist failed:", err));
            return;
        }
        openPlaylist(playlistId, Boolean(e.target.closest(".music-album-play-btn")));
        return;
    }
    const play = Boolean(e.target.closest(".music-album-play-btn"));
    openAlbum(parseInt(card.dataset.cdId, 10), play);
});
musicGrid.addEventListener("keydown", (e) => {
    if (e.key !== "Enter" && e.key !== " ") return;
    const groupHeader = e.target.closest(".author-group-header");
    if (groupHeader) return; // ボタンのデフォルト動作に任せてクリック扱いにする
    const card = e.target.closest(".music-album");
    if (!card) return;
    e.preventDefault();
    if (card.dataset.playlistId) openPlaylist(card.dataset.playlistId);
    else openAlbum(parseInt(card.dataset.cdId, 10), false);
});

// 表示切り替え (アーティスト別 / 一覧)
document.querySelectorAll(".view-tab").forEach((tab) => {
    tab.addEventListener("click", () => {
        document.querySelectorAll(".view-tab").forEach((t) => {
            t.classList.toggle("active", t === tab);
            t.setAttribute("aria-selected", String(t === tab));
        });
        currentView = tab.dataset.view;
        localStorage.setItem("dantalian_music_view", currentView);
        renderGrid();
    });
});

// 並び順 (登録順 / タイトル順 / 発売日順)
document.getElementById("music-sort-buttons").addEventListener("click", (e) => {
    const btn = e.target.closest(".width-btn");
    if (!btn) return;
    currentSort = btn.dataset.sort;
    localStorage.setItem("dantalian_music_sort", currentSort);
    document.querySelectorAll("#music-sort-buttons .width-btn").forEach((b) => {
        b.classList.toggle("active", b.dataset.sort === currentSort);
    });
    renderGrid();
});

// フィルタ
document.querySelector(".music-filters").addEventListener("click", (e) => {
    const btn = e.target.closest(".music-filter");
    if (!btn) return;
    currentFilter = btn.dataset.filter;
    document.querySelectorAll(".music-filter").forEach((b) => {
        b.classList.toggle("active", b.dataset.filter === currentFilter);
    });
    renderGrid();
});

function renderMusicLoadError(messages) {
    musicCount.textContent = "";
    musicGrid.innerHTML = `
        <div class="load-error" role="alert">
            <span class="material-icons" aria-hidden="true">cloud_off</span>
            <p>音楽ライブラリを読み込めませんでした。登録内容は削除されていません。</p>
            <small>${escapeHtml(messages.join(" / "))}</small>
            <button type="button" class="btn btn-ghost" onclick="initMusicPage()">再読み込み</button>
        </div>`;
}

(function initSortButtons() {
    document.querySelectorAll("#music-sort-buttons .width-btn").forEach((btn) => {
        btn.classList.toggle("active", btn.dataset.sort === currentSort);
    });
})();

(function initViewTabs() {
    document.querySelectorAll(".view-tab").forEach((tab) => {
        tab.classList.toggle("active", tab.dataset.view === currentView);
        tab.setAttribute("aria-selected", String(tab.dataset.view === currentView));
    });
})();

// 起動
async function initMusicPage() {
    await loadAudioDataSaverPolicy();
    let cdsLoaded = true;
    try {
        const res = await fetch("/api/cds");
        if (!res.ok) throw new Error(`CD一覧: HTTP ${res.status}`);
        allAlbums = await res.json();
    } catch (error) {
        console.error("music cds load failed:", error);
        allAlbums = [];
        cdsLoaded = false;
    }
    const playlistsLoaded = await loadPlaylists();
    if (!cdsLoaded || !playlistsLoaded) {
        const messages = [];
        if (!cdsLoaded) messages.push("CD/オーディオブック一覧の取得に失敗");
        if (!playlistsLoaded) messages.push(`プレイリスト: ${playlistLoadError || "取得に失敗"}`);
        renderMusicLoadError(messages);
        return;
    }
    window.musicAlbums = allAlbums;
    player.setAlbums(playableAlbums());
    renderGrid();

    // ?play={cdId} があれば自動再生 (CD詳細の「再生」ボタンから遷移)
    const playId = parseInt(new URLSearchParams(location.search).get("play"), 10);
    if (!isNaN(playId) && playableAlbums().some((c) => c.id === playId)) {
        openAlbum(playId, true);
    }

    const playlistId = parseInt(new URLSearchParams(location.search).get("playlist"), 10);
    if (!isNaN(playlistId)) openPlaylist(playlistId);

    const editPlaylistId = parseInt(new URLSearchParams(location.search).get("edit_playlist"), 10);
    if (!isNaN(editPlaylistId)) openPlaylistEditor(editPlaylistId);
}

initMusicPage();
