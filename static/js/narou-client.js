// narou.rs の書誌情報と EPUB はブラウザが直接取得する。
//
// Dantalian は接続情報・対象タグと EPUB 宛先の 302 だけを用意する。
// カタログはサーバーにも端末にも永続保存しない。EPUB だけは端末側の
// 容量制限付きキャッシュを使用でき、鮮度は API の last_update で確認する。
//
// 認証は narou.rs の既存ユーザー認証（Cloudflare Access セッション /
// ネイティブの UI セッション）をそのまま使う。access_token のような新しい
// 認証は発行しない。
(function initNarouClient() {
    // お気に入りタグは全タグではなく favorite_tags だけを読む。
    const FAVOURITE_TAGS_PATH = "/favorite-tags";

    let catalogueRequest = null;

    // Dantalian の接続情報（認証は含まれない）。
    async function fetchConnection() {
        const response = await fetch("/api/narou/connection", { cache: "no-store" });
        const payload = await response.json().catch(() => null);
        if (!response.ok) {
            throw new Error(payload?.error || `narou 接続情報の取得に失敗しました（HTTP ${response.status}）。`);
        }
        if (!payload || typeof payload !== "object") {
            throw new Error("narou 接続情報の応答が不正です。");
        }
        return payload;
    }

    function readUrl(connection, path, params) {
        let url;
        try {
            url = new URL(`${connection.api_base}${path}`);
        } catch {
            throw new Error("narou.rs の接続先 URL が不正です。");
        }
        for (const [key, value] of Object.entries(params || {})) {
            if (value != null) url.searchParams.set(key, String(value));
        }
        return url.href;
    }

    function upstreamError(payload, status) {
        const error = payload && payload.error;
        if (error && typeof error === "object" && typeof error.message === "string" && error.message) {
            return error.message;
        }
        if (typeof error === "string" && error) return error;
        if (payload && typeof payload.message === "string" && payload.message) return payload.message;
        if (status === 401 || status === 403) return "narou.rs の認証が必要です。";
        return `narou.rs の応答が不正です（HTTP ${status}）。`;
    }

    // 既存ユーザー認証で直接読む。長期認証ヘッダーも access_token も載せない。
    async function readJson(connection, path, params) {
        const response = await fetch(readUrl(connection, path, params), {
            cache: "no-store",
            credentials: "include",
            referrerPolicy: "no-referrer",
        });
        const payload = await response.json().catch(() => null);
        if (!response.ok) {
            const error = new Error(upstreamError(payload, response.status));
            error.status = response.status;
            error.authRequired = response.status === 401 || response.status === 403;
            throw error;
        }
        if (!payload || typeof payload !== "object") {
            const error = new Error("narou.rs の応答が JSON ではありません（ログインが必要な場合があります）。");
            error.authRequired = true;
            throw error;
        }
        return payload;
    }

    async function fetchFavouriteTags(connection) {
        const payload = await readJson(connection, FAVOURITE_TAGS_PATH);
        if (payload.success === false) {
            throw new Error(payload.message || "narou.rs のお気に入りタグを取得できませんでした。");
        }
        if (payload.success !== true || !Array.isArray(payload.tags)) {
            throw new Error("お気に入りタグの応答が不正です。");
        }
        return payload.tags.filter((tag) => typeof tag === "string" && tag.trim());
    }

    async function fetchNovels(connection) {
        const payload = await readJson(connection, "/novels", { limit: "all" });
        if (!Array.isArray(payload.data)) throw new Error("小説一覧の応答が不正です。");
        return payload.data;
    }

    // 指定タグとお気に入りタグの和集合（完全一致）。
    function selectTags(connection, favourites, useFavourites) {
        const tags = new Set();
        const configured = Array.isArray(connection.tags) ? connection.tags : [];
        for (const tag of configured) {
            if (typeof tag === "string" && tag.trim()) tags.add(tag.trim());
        }
        if (useFavourites) {
            for (const tag of favourites || []) {
                if (typeof tag === "string" && tag.trim()) tags.add(tag.trim());
            }
        }
        return tags;
    }

    // 部分一致ではなく完全一致で判定する。
    function matchesTags(novel, tags) {
        if (!novel || !Array.isArray(novel.tags)) return false;
        return novel.tags.some((tag) => typeof tag === "string" && tags.has(tag.trim()));
    }

    function datePart(value) {
        if (typeof value !== "string") return "";
        const match = /^(\d{4}-\d{2}-\d{2})/.exec(value);
        return match ? match[1] : "";
    }

    // リンクに使う URL は HTTP(S) だけ許可する。
    function safeHttpUrl(value) {
        if (typeof value !== "string" || !value) return "";
        try {
            const url = new URL(value);
            return url.protocol === "http:" || url.protocol === "https:" ? url.href : "";
        } catch {
            return "";
        }
    }

    // 画面用の項目へ変換する。cover_url は常に null（カバーは配信されない）。
    function catalogueItem(novel, connection) {
        const id = Number(novel.id);
        const author = typeof novel.author === "string" ? novel.author.trim() : "";
        const tags = Array.isArray(novel.tags)
            ? novel.tags.filter((tag) => typeof tag === "string")
            : [];
        return {
            id: `narou:${id}`,
            narou_id: id,
            sourceType: "narou",
            originalId: id,
            title: typeof novel.title === "string" ? novel.title : "",
            cover_url: null,
            authors: author ? [{ name: author, sort_order: 0 }] : [],
            primary_author_name: author || null,
            publisher: typeof novel.sitename === "string" ? novel.sitename : "",
            tags,
            toc_url: safeHttpUrl(novel.toc_url),
            display_url: safeHttpUrl(novel.display_url),
            last_update: typeof novel.last_update === "string" ? novel.last_update : null,
            general_lastup: typeof novel.general_lastup === "string" ? novel.general_lastup : null,
            length: novel.length ?? null,
            general_all_no: novel.general_all_no ?? null,
            series_id: null,
            media_type: "book",
            copies_count: 0,
            lent_count: 0,
            publish_date: datePart(novel.general_lastup) || datePart(novel.last_update),
            source: connection && connection.source ? String(connection.source) : "",
        };
    }

    // カタログ全体を取得する。EPUB の取得は行わない。
    async function loadCatalogue({ verifyConnection = false } = {}) {
        const connection = await fetchConnection();
        if (!connection.configured) {
            const result = { state: "disabled", items: [], tags: [], connection };
            client.items = result.items;
            client.state = result.state;
            return result;
        }
        try {
            // お気に入りタグを使うかどうかはサーバー側と同じ既定値に従う。
            const useFavourites = connection.use_favorites !== false;
            const favourites = useFavourites || verifyConnection ? await fetchFavouriteTags(connection) : [];
            const tags = selectTags(connection, favourites, useFavourites);
            if (tags.size === 0) {
                // 対象タグが無いときは一覧を取得しない。
                const result = { state: "ready", items: [], tags: [], connection };
                client.items = result.items;
                client.state = result.state;
                return result;
            }
            const novels = await fetchNovels(connection);
            const items = novels
                .filter((novel) => matchesTags(novel, tags))
                .map((novel) => catalogueItem(novel, connection));
            const result = { state: "ready", items, tags: [...tags], connection };
            client.items = items;
            client.state = result.state;
            return result;
        } catch (error) {
            // 認証・通信の失敗からログインリンクを出せるようにする。
            error.connection = connection;
            throw error;
        }
    }

    // 同じ要求を重ねない。
    function loadCatalogueOnce() {
        if (!catalogueRequest) {
            catalogueRequest = loadCatalogue().finally(() => {
                catalogueRequest = null;
            });
        }
        return catalogueRequest;
    }

    // 小説 1 件の書誌情報を直接取得する。
    async function loadNovel(id) {
        const novelId = Number(id);
        if (!Number.isInteger(novelId) || novelId < 0) {
            throw new Error("小説 ID が不正です。");
        }
        const connection = await fetchConnection();
        if (!connection.configured) {
            throw new Error("narou.rs が設定されていません。");
        }
        try {
            const novel = await readJson(connection, `/novels/${encodeURIComponent(String(novelId))}`);
            return { novel, connection };
        } catch (error) {
            // 認証・通信の失敗からログインリンクを出せるようにする。
            error.connection = connection;
            throw error;
        }
    }

    // EPUB は Dantalian の 302 発行 URL を辿るだけ（Dantalian は中継しない）。
    function epubUrl(id) {
        return `/api/narou/novels/${encodeURIComponent(String(id))}/epub`;
    }

    // 読み込み位置を保つための安定した書籍キー。
    function bookKey(id, connection) {
        const source = String((connection && connection.source) || "").replace(/\/+$/, "");
        return `narou:${id}@${source}`;
    }

    // 内容の更新を検知するための安定した版番号。URL やトークンを含めない。
    function contentHash(novel) {
        const revision = [
            novel.last_update || "",
            novel.general_lastup || "",
            novel.length ?? "",
            novel.general_all_no ?? "",
        ].join("|");
        return `narou-${novel.id}-${fnv1a(revision)}`;
    }

    function fnv1a(text) {
        let hash = 0x811c9dc5;
        for (let index = 0; index < text.length; index += 1) {
            hash ^= text.charCodeAt(index);
            hash = Math.imul(hash, 0x01000193) >>> 0;
        }
        return hash.toString(16).padStart(8, "0");
    }

    // 認証が必要なときの利用者向けリンク（ログイン画面）。
    function openUrl(connection) {
        const source = String((connection && connection.source) || "").trim();
        return source ? source.replace(/\/+$/, "") : "";
    }

    const client = {
        // 画面に出す Web 小説の一覧（メモリ上のみ）。
        items: [],
        state: "idle",
        loadCatalogue,
        loadCatalogueOnce,
        loadNovel,
        fetchConnection,
        epubUrl,
        bookKey,
        contentHash,
        catalogueItem,
        openUrl,
    };

    window.narouClient = client;
})();
