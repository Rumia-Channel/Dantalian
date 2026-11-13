// The standalone reader checks current metadata before reusing device-local EPUB bytes.
(function initializeEpubReader() {
    const bootstrap = document.currentScript;
    const version = new URL(bootstrap.src).searchParams.get("v");
    const readerSource = new URL(bootstrap.dataset.bibiSource, location.href);
    const state = { version, readerVersion: readerSource.searchParams.get("v"), descriptor: null, cacheStatus: "disabled", warning: "" };
    window.dantalianEpubReader = state;

    function failure(message, status) {
        const error = new Error(message);
        if (status != null) error.status = status;
        return error;
    }

    async function readBook(id) {
        const response = await fetch(`/api/books/${id}`, { cache: "no-store", credentials: "same-origin" });
        if (!response.ok) throw failure(`書籍情報を取得できませんでした（HTTP ${response.status}）。`, response.status);
        try { return await response.json(); }
        catch { throw failure("書籍情報を取得できませんでした。Dantalian へのログインを確認してください。"); }
    }

    async function describe(parameters) {
        const resource = new URL(parameters.get("book"), location.href);
        if (resource.origin !== location.origin) return null;
        const narou = /^\/api\/narou\/novels\/(0|[1-9]\d*)\/epub$/.exec(resource.pathname);
        if (narou) {
            const id = Number(narou[1]);
            const current = await window.narouClient.loadNovel(id);
            const key = window.narouClient.bookKey(id, current.connection);
            parameters.set("book-key", key);
            parameters.set("content-hash", window.narouClient.contentHash(current.novel));
            parameters.set("storage-namespace", "narou");
            const revision = current.novel.last_update;
            const revisionTime = typeof revision === "string" ? Date.parse(revision) : NaN;
            if (!Number.isFinite(revisionTime)) {
                state.warning = "last_update を確認できないため、この作品の EPUB は端末にキャッシュしません。";
                return null;
            }
            return {
                key, revision, revisionTime, url: resource.href, credentials: "include",
                async unchanged() {
                    const latest = await window.narouClient.loadNovel(id);
                    return window.narouClient.bookKey(id, latest.connection) === key && latest.novel.last_update === revision;
                },
            };
        }
        const file = /^\/epubs\/([^/]+\.epub)$/i.exec(resource.pathname);
        const book = /^book:([1-9]\d*)$/.exec(parameters.get("book-key") || "");
        if (!file || !book) return null;
        const hash = decodeURIComponent(file[1]);
        const metadata = await readBook(book[1]);
        if (!metadata.files?.some(item => item.file_hash === hash)) {
            throw failure("この EPUB は書籍に登録されていません。本一覧から開き直してください。", 404);
        }
        parameters.set("content-hash", hash);
        return { key: `epub:${hash}`, revision: hash, revisionTime: null, url: resource.href, credentials: "same-origin" };
    }

    async function download(descriptor) {
        const response = await fetch(descriptor.url, {
            credentials: descriptor.credentials, cache: "no-store", referrerPolicy: "no-referrer",
        });
        if (response.status !== 200) {
            const error = failure(`EPUB を取得できませんでした（HTTP ${response.status} ${response.statusText}）。`, response.status);
            await response.body?.cancel();
            throw error;
        }
        return response.blob();
    }

    state.consume = async function consume(parse) {
        const descriptor = state.descriptor;
        const operation = async () => {
            let cached;
            try { cached = await window.epubCache.read(descriptor.key, descriptor.revision); }
            catch { state.warning = "端末キャッシュを読み込めなかったため、EPUB を再取得しました。"; }
            if (cached) {
                state.cacheStatus = "hit";
                try { await parse(cached.blob); }
                catch (error) {
                    await window.epubCache.remove(descriptor.key, descriptor.revision).catch(() => {});
                    throw error;
                }
                return;
            }
            state.cacheStatus = "miss";
            const blob = await download(descriptor);
            const isEpub = await parse(blob);
            if (!isEpub) return;
            if (descriptor.unchanged) {
                try {
                    if (!await descriptor.unchanged()) {
                        state.cacheStatus = "changed";
                        state.warning = "取得中に作品が更新されたため、今回の EPUB はキャッシュしませんでした。";
                        return;
                    }
                } catch {
                    state.cacheStatus = "unverified";
                    state.warning = "更新日時を再確認できなかったため、今回の EPUB はキャッシュしませんでした。";
                    return;
                }
            }
            try {
                const result = await window.epubCache.put({
                    key: descriptor.key, revision: descriptor.revision, revisionTime: descriptor.revisionTime, blob,
                }, state.generation);
                state.cacheStatus = result.stored ? "stored" : result.reason;
                if (result.reason === "oversize") state.warning = "この EPUB は容量上限を超えるため、端末にキャッシュしませんでした。";
            } catch {
                state.cacheStatus = "error";
                state.warning = "端末の保存容量などの制約で EPUB をキャッシュできませんでした。読書は続けられます。";
            }
        };
        // A second tab rechecks the cache after the first tab finishes saving the same book.
        return navigator.locks ? navigator.locks.request(`dantalian-epub:${descriptor.key}`, operation) : operation();
    };

    function showError(error) {
        const area = document.createElement("main");
        area.setAttribute("role", "alert");
        const message = document.createElement("p");
        message.textContent = `本を開けませんでした。${error.message || error}`;
        const back = document.createElement("a");
        back.href = "/books/";
        back.textContent = "本一覧に戻る";
        area.append(message, back);
        const source = window.narouClient?.openUrl(error.connection);
        if (source) {
            const login = document.createElement("a");
            login.href = source;
            login.target = "_blank";
            login.rel = "noopener noreferrer";
            login.textContent = "narou.rs でログイン";
            area.append(" ", login);
        }
        document.body.replaceChildren(area);
    }

    async function start() {
        try {
            let settings;
            try { settings = await window.epubCache.settings(); }
            catch { state.warning = "このブラウザでは EPUB の端末キャッシュを利用できません。"; }
            const parameters = new URLSearchParams(location.search);
            if (settings?.limitBytes > 0 && parameters.get("book")) {
                state.generation = settings.generation;
                state.descriptor = await describe(parameters);
                const address = new URL(location.href);
                address.search = parameters;
                if (address.href !== location.href) history.replaceState(history.state, "", address);
            }
            const script = document.createElement("script");
            script.id = "bibi-script";
            script.src = readerSource.href;
            script.dataset.bibiPreset = "dantalian";
            script.dataset.bibiExtensions = "";
            script.onload = () => {
                // The host loads Bibi after DOMContentLoaded, then starts its normal lifecycle once.
                Bibi.ring().then(() => {
                    if (state.warning && Bibi.API?.getState().status === "ready") {
                        I.notify(state.warning, { Time: 10000 });
                    }
                });
            };
            script.onerror = () => showError(failure("Bibi を読み込めませんでした。"));
            document.head.appendChild(script);
        } catch (error) { showError(error); }
    }

    if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start, { once: true });
    else start();
})();
