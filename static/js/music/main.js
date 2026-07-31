// 音楽ライブラリページ: CD/オーディオブックの一覧表示と、インラインプレイヤーの起動。

const musicGrid = document.getElementById("music-grid");
const musicCount = document.getElementById("music-count");
const playerRoot = document.getElementById("music-player-root");

let allAlbums = [];           // 再生可能トラックを持つ CD 一覧
let currentFilter = "all";

const player = createPlayerUI(playerRoot);
window.musicPlayer = player;

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

    const albumHtml = media.albums.map((cd) => {
        const type = albumMediaType(cd);
        const badge = type === "audiobook"
            ? '<span class="music-album-badge music-album-badge--audiobook">AB</span>'
            : '<span class="music-album-badge">CD</span>';
        const cover = cd.cover_url
            ? `<img class="music-album-cover" src="/images/${cd.cover_url}" alt="${escapeAttr(cd.title)}" loading="lazy">`
            : `<div class="music-album-coverfallback"><span class="material-icons">album</span></div>`;
        const trackCount = (cd.tracks || []).filter((t) => t.file_hash).length;
        const artist = cd.artist ? escapeHtml(cd.artist) : "&nbsp;";
        return `
        <div class="music-album" data-cd-id="${cd.id}" tabindex="0" role="button" aria-label="${escapeAttr(cd.title)} を再生">
            <div class="music-album-coverwrap">
                ${cover}
                ${badge}
                <div class="music-album-play" data-album-action="play">
                    <span class="music-album-play-btn"><span class="material-icons">play_arrow</span></span>
                </div>
            </div>
            <div class="music-album-name">${escapeHtml(cd.title)}</div>
            <div class="music-album-artist">${artist}</div>
            <div class="music-album-meta">${trackCount} 曲</div>
        </div>`;
    }).join("");
    musicGrid.innerHTML = albumHtml + media.playlists.map(renderPlaylistCard).join("");
}

function openAlbum(cdId, autoplay) {
    player.setAlbums(playableAlbums());
    player.openAlbum(cdId, !!autoplay);
}

// グリッド操作 (クリック / キーボード) — 自動再生しない
musicGrid.addEventListener("click", (e) => {
    const card = e.target.closest(".music-album");
    if (!card) return;
    const playlistId = card.dataset.playlistId;
    if (playlistId) {
        openPlaylist(playlistId);
        return;
    }
    const play = Boolean(e.target.closest("[data-album-action=\"play\"]"));
    openAlbum(parseInt(card.dataset.cdId, 10), play);
});
musicGrid.addEventListener("keydown", (e) => {
    if (e.key !== "Enter" && e.key !== " ") return;
    const card = e.target.closest(".music-album");
    if (!card) return;
    e.preventDefault();
    if (card.dataset.playlistId) openPlaylist(card.dataset.playlistId);
    else openAlbum(parseInt(card.dataset.cdId, 10), false);
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

// 起動
(async function init() {
    try {
        const res = await fetch("/api/cds");
        allAlbums = await res.json();
    } catch {
        allAlbums = [];
    }
    await loadPlaylists();
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
})();
