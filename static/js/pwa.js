// PWA: Service Worker 登録。
// 登録失敗してもアプリ動作に影響しないため、エラーは握りつぶす。

(function () {
    if (!("serviceWorker" in navigator)) return;
    window.addEventListener("load", () => {
        navigator.serviceWorker.register("/sw.js").catch(() => {});
    });
})();
