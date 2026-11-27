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

    // params は [key, value] のペア列。`tag` / `not_tag` の繰り返しを
    // サーバーへ届けるため、URLSearchParams#set ではなく append で組み立てる。
    function readUrl(connection, path, params) {
        let url;
        try {
            url = new URL(`${connection.api_base}${path}`);
        } catch {
            throw new Error("narou.rs の接続先 URL が不正です。");
        }
        for (const [key, value] of params || []) {
            if (value != null) url.searchParams.append(key, String(value));
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

    // タグ条件のパース。設定の 1 行が 1 つの条件 (メンバー) で、行どうしは
    // OR。行の中では空白区切りが AND、`|` が OR、先頭の `-` が除外。
    //
    //   "sf ファンタジー"   → sf AND ファンタジー
    //   "sf|ファンタジー"   → sf OR ファンタジー
    //   "sf -r15"           → sf AND NOT r15
    //   "sf", "恋愛" (2 行) → sf OR 恋愛
    //
    // 空白を含むタグは "..." で囲む。`|` を含むタグは表現できない
    // (narou.rs 側の OR 区切りと衝突するため)。

    // 引用符を意識して空白で分割する。
    function splitTerms(text) {
        const terms = [];
        let current = "";
        let quoted = false;
        for (const ch of text) {
            if (ch === '"') {
                quoted = !quoted;
                current += ch;
                continue;
            }
            if (/\s/.test(ch) && !quoted) {
                if (current.trim()) terms.push(current.trim());
                current = "";
                continue;
            }
            current += ch;
        }
        if (current.trim()) terms.push(current.trim());
        return terms;
    }

    function stripQuotes(value) {
        if (value.length >= 2 && value.startsWith('"') && value.endsWith('"')) {
            return value.slice(1, -1);
        }
        return value;
    }

    // `|` 区切りの OR 候補へ分ける (引用符の中の `|` は区切らない)。
    // narou.rs 側では `|` を含むタグを表現できないため、引用符の有無に
    // かかわらず `|` を含む候補は残さない。
    function splitAlternatives(value) {
        const alternatives = [];
        let current = "";
        let quoted = false;
        for (const ch of value) {
            if (ch === '"') {
                quoted = !quoted;
                current += ch;
                continue;
            }
            if (ch === "|" && !quoted) {
                pushAlternative(alternatives, current);
                current = "";
                continue;
            }
            current += ch;
        }
        pushAlternative(alternatives, current);
        return alternatives;
    }

    function pushAlternative(alternatives, raw) {
        const tag = stripQuotes(raw.trim()).trim();
        if (tag && !tag.includes("|") && !alternatives.includes(tag)) {
            alternatives.push(tag);
        }
    }

    // 設定行の配列をクエリ用の構造へ変換する。
    // 返り値: { members: [{ groups: [[tag,...],...], notTags: [...] }], exclusions: [...] }
    // exclusions は「`-tag` だけの行」の全体除外。メンバー内の `-tag` は
    // そのメンバーだけに効く notTags。
    function parseTagQuery(lines) {
        const members = [];
        const exclusions = [];
        const seenMembers = new Set();
        for (const rawLine of lines || []) {
            if (typeof rawLine !== "string" || !rawLine.trim()) continue;
            const groups = [];
            const notTags = [];
            for (const term of splitTerms(rawLine.trim())) {
                const negated = term.startsWith("-") || term.startsWith("^") || term.startsWith("!");
                const body = negated ? term.slice(1) : term;
                const alternatives = splitAlternatives(body);
                if (!alternatives.length) continue;
                if (negated) {
                    for (const tag of alternatives) {
                        if (!notTags.includes(tag)) notTags.push(tag);
                    }
                } else {
                    groups.push(alternatives);
                }
            }
            if (!groups.length) {
                // 否定だけの行は全体の除外として扱う。
                for (const tag of notTags) {
                    if (!exclusions.includes(tag)) exclusions.push(tag);
                }
                continue;
            }
            const key = JSON.stringify([groups, notTags]);
            if (seenMembers.has(key)) continue;
            seenMembers.add(key);
            members.push({ groups, notTags });
        }
        return { members, exclusions };
    }

    // メンバー (設定 1 行) に対応するクエリパラメータ。`tag` は繰り返しで
    // AND、`|` 区切りで OR、`not_tag` は完全一致の除外 (narou.rs 0.6.14
    // 以降)。URL のどのパラメータへ落ちるかをテストできるよう純粋関数にする。
    function memberParams(member, globalExclusions) {
        const params = [["limit", "all"]];
        for (const group of member.groups) {
            params.push(["tag", group.join("|")]);
        }
        const exclusions = [];
        for (const tag of [...member.notTags, ...(globalExclusions || [])]) {
            if (!exclusions.includes(tag)) exclusions.push(tag);
        }
        if (exclusions.length) {
            params.push(["not_tag", exclusions.join("|")]);
        }
        return params;
    }

    // 1 メンバー (= 設定 1 行) 分の一覧をサーバー側の絞り込みで取得する。
    // 古い narou.rs は `|` 入りの完全一致や未知の not_tag を正しく扱えない
    // ため、受け取った行はクライアント側でも完全一致で再検証して過不足を防ぐ。
    async function fetchMemberNovels(connection, member, globalExclusions) {
        const payload = await readJson(connection, "/novels", memberParams(member, globalExclusions));
        if (!Array.isArray(payload.data)) throw new Error("小説一覧の応答が不正です。");
        return payload.data.filter((novel) => matchesMember(novel, member, globalExclusions));
    }

    // 完全一致の再検証。サーバーが返した行だけに適用する。
    function matchesMember(novel, member, globalExclusions) {
        if (!novel || !Array.isArray(novel.tags)) return false;
        const tags = novel.tags.filter((tag) => typeof tag === "string").map((tag) => tag.trim());
        for (const group of member.groups) {
            if (!group.some((wanted) => tags.includes(wanted))) return false;
        }
        for (const excluded of [...member.notTags, ...globalExclusions]) {
            if (tags.includes(excluded)) return false;
        }
        return true;
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
    //
    // 絞り込みは narou.rs 側のクエリで行う (`tag` / `not_tag`)。設定行ごと
    // (行どうしは OR) に 1 クエリを投げ、作品 ID で和集合を取る。全件を
    // 落としてクライアントで選別する従来の方法は、ライブラリが大きいと
    // API 全体が重く応答しないため使わない。
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
            const query = parseTagQuery(Array.isArray(connection.tags) ? connection.tags : []);
            if (useFavourites) {
                // お気に入りタグは 1 タグ 1 行 (= OR の和集合) として足す。
                const seen = new Set(query.members.map((member) => JSON.stringify([member.groups, member.notTags])));
                for (const tag of favourites) {
                    if (typeof tag !== "string" || !tag.trim() || tag.includes("|")) continue;
                    const groups = [[tag.trim()]];
                    const key = JSON.stringify([groups, []]);
                    if (seen.has(key)) continue;
                    seen.add(key);
                    query.members.push({ groups, notTags: [] });
                }
            }
            if (!query.members.length) {
                // 対象タグが無いときは一覧を取得しない。
                const result = { state: "ready", items: [], tags: [], connection };
                client.items = result.items;
                client.state = result.state;
                return result;
            }
            const perMember = await Promise.all(
                query.members.map((member) => fetchMemberNovels(connection, member, query.exclusions)),
            );
            // 作品 ID で和集合を取る (narou ID 昇順で安定させる)。
            const novelsById = new Map();
            for (const novels of perMember) {
                for (const novel of novels) {
                    const id = Number(novel && novel.id);
                    if (Number.isFinite(id) && !novelsById.has(id)) novelsById.set(id, novel);
                }
            }
            const items = [...novelsById.keys()]
                .sort((a, b) => a - b)
                .map((id) => catalogueItem(novelsById.get(id), connection));
            const tags = (Array.isArray(connection.tags) ? connection.tags : [])
                .filter((line) => typeof line === "string" && line.trim());
            if (useFavourites && favourites.length) tags.push("お気に入りタグ");
            const result = { state: "ready", items, tags, connection };
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
        // 条件の解釈をブラウザから確かめられるよう公開する (テスト用)。
        parseTagQuery,
        matchesMember,
        memberParams,
        catalogueItem,
        openUrl,
    };

    window.narouClient = client;
})();
