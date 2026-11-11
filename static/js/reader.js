(async function openStandaloneReader() {
    const parameters = new URLSearchParams(location.search);
    const bookId = parameters.get("book");
    const narouId = parameters.get("narou");
    const status = document.getElementById("reader-status");
    const message = document.getElementById("reader-status-message");
    const back = document.getElementById("reader-back");
    const edit = document.getElementById("reader-edit");
    const download = document.getElementById("reader-download");

    const requestedReturn = parameters.get("from") || "/books/";
    const returnUrl = new URL(URL.canParse(requestedReturn, location.origin) ? requestedReturn : "/books/", location.origin);
    let returnPath = "/books/";
    if (!/[\\\u0000-\u001f\u007f]/.test(requestedReturn) &&
        returnUrl.origin === location.origin && ["/", "/books/", "/edit/"].includes(returnUrl.pathname)) {
        returnPath = returnUrl.pathname + returnUrl.search + returnUrl.hash;
        back.href = returnPath;
        back.textContent = returnUrl.pathname === "/" ? "一覧に戻る" : returnUrl.pathname === "/edit/" ? "編集に戻る" : "本一覧に戻る";
    }

    // Web 小説 (narou.rs) は書誌情報をブラウザが直接取得し、EPUB も
    // narou.rs から直接読む（Dantalian は 302 を発行するだけ）。
    async function openNarouReader(id, from) {
        if (!/^[1-9]\d*$/.test(id)) throw new Error("小説 ID が正しくありません。");
        const client = window.narouClient;
        if (!client) throw new Error("narou クライアントを読み込めませんでした。");
        const novelId = Number(id);
        const { novel, connection } = await client.loadNovel(novelId);
        document.getElementById("reader-title").textContent = novel.title || `小説 ${novelId}`;
        const readerUrl = new URL("/vendor/bibi/dantalian.html", location.origin);
        const readerParameters = new URLSearchParams({
            book: client.epubUrl(novelId),
            "book-key": client.bookKey(novelId, connection),
            "content-hash": client.contentHash(novel),
            "storage-namespace": "narou",
            "storage-user": "local",
            "host-return-to": from,
        });
        const entry = parameters.get("entry");
        if (entry === "start" || entry === "end") {
            readerParameters.set("edge", entry === "start" ? "head" : "foot");
        }
        readerUrl.search = readerParameters;
        // Replace the launcher so browser Back returns directly to the catalogue.
        location.replace(readerUrl.href);
    }

    try {
        if (narouId) {
            await openNarouReader(narouId, returnPath);
            return;
        }
        if (!bookId || !/^[1-9]\d*$/.test(bookId)) throw new Error("読書する本が指定されていません。");
        edit.href = `/edit/?mode=book&book=${encodeURIComponent(bookId)}`;
        edit.classList.remove("hidden");
        const response = await fetch(`/api/books/${encodeURIComponent(bookId)}`, { cache: "no-store" });
        if (!response.ok) {
            throw new Error(response.status === 404 ? "本が見つかりません。" : `書籍情報を取得できませんでした（HTTP ${response.status}）。`);
        }
        const book = await response.json();
        document.getElementById("reader-title").textContent = book.title;
        const fileId = parameters.get("file");
        if (fileId != null && !/^[1-9]\d*$/.test(fileId)) throw new Error("ファイル指定が正しくありません。");
        const files = book.files || [];
        if (!files.length) throw new Error("この本にはファイルが登録されていません。「書籍情報」からファイルを登録してください。");
        // A catalogue click opens the oldest attachment; explicit links select one.
        const file = fileId == null ? files[0] : files.find((file) => String(file.id) === fileId);
        if (!file) throw new Error("指定されたファイルが見つかりません。");
        const bookUrl = `/epubs/${encodeURIComponent(file.file_hash)}`;
        download.href = bookUrl;
        download.download = file.file_name;
        download.classList.remove("hidden");

        const readerUrl = new URL("/vendor/bibi/dantalian.html", location.origin);
        const readerParameters = new URLSearchParams({
            book: bookUrl,
            "book-key": `book:${book.id}`,
            "content-hash": file.file_hash,
            "storage-namespace": "dantalian",
            "storage-user": "local",
            "host-return-to": returnPath,
        });
        const entry = parameters.get("entry");
        if (entry === "start" || entry === "end") {
            readerParameters.set("edge", entry === "start" ? "head" : "foot");
        }
        readerUrl.search = readerParameters;
        // Replace the launcher so browser Back returns directly to the catalogue.
        location.replace(readerUrl.href);
    } catch (error) {
        status.setAttribute("role", "alert");
        message.textContent = `本を開けませんでした。${error.message}`;
        // 認証が必要なときは、narou.rs でのログインを案内する。
        const source = window.narouClient?.openUrl(error.connection);
        if (source) {
            const anchor = document.createElement("a");
            anchor.href = source;
            anchor.target = "_blank";
            anchor.rel = "noopener noreferrer";
            anchor.textContent = "narou.rs でログイン";
            message.append(" ", anchor);
        }
    }
})();
