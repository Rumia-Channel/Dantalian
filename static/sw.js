// PWA インストール要件を満たすための最小 Service Worker。
// オフラインキャッシュは行わない (音声・書籍データはサーバー依存のため)。
// 静的アセットは ?v=ASSET_VERSION のキャッシュバスタで鮮度が管理されている。

self.addEventListener("install", (event) => {
    event.waitUntil(self.skipWaiting());
});

self.addEventListener("activate", (event) => {
    event.waitUntil(self.clients.claim());
});
