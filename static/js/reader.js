(async function initializeReader() {
    const parameters = new URLSearchParams(location.search);
    const bookId = parameters.get("book");
    const status = document.getElementById("reader-status");
    const message = document.getElementById("reader-status-message");
    const progress = document.getElementById("reader-progress");
    const container = document.getElementById("reader-frame");
    const back = document.getElementById("reader-back");
    const edit = document.getElementById("reader-edit");
    const download = document.getElementById("reader-download");
    let bucket = null;
    let session = null;
    let closed = false;

    const requestedReturn = parameters.get("from") || "/books/";
    const returnUrl = new URL(URL.canParse(requestedReturn, location.origin) ? requestedReturn : "/books/", location.origin);
    if (returnUrl.origin === location.origin && ["/", "/books/", "/edit/"].includes(returnUrl.pathname)) {
        back.href = returnUrl.pathname + returnUrl.search;
        back.textContent = returnUrl.pathname === "/" ? "一覧に戻る" : returnUrl.pathname === "/edit/" ? "編集に戻る" : "本一覧に戻る";
    }

    function dispose() {
        closed = true;
        if (bucket) {
            bucket.api.dispose();
            bucket.Frame?.remove();
            bucket.Anchor.remove();
            bucket = null;
        }
    }

    window.addEventListener("pagehide", dispose, { once: true });
    // A disposed bridge cannot be reused when the browser restores its page cache.
    window.addEventListener("pageshow", (event) => {
        if (event.persisted) location.reload();
    });

    function updateProgress(position) {
        if (Number.isFinite(position?.progress)) {
            progress.value = `${Math.round(position.progress * 1000) / 10}%`;
        }
    }

    try {
        if (!bookId || !/^[1-9]\d*$/.test(bookId)) throw new Error("読書する本が指定されていません。");
        edit.href = `/edit/?mode=book&book=${encodeURIComponent(bookId)}`;
        edit.classList.remove("hidden");
        const response = await fetch(`/api/books/${encodeURIComponent(bookId)}`, { cache: "no-store" });
        if (!response.ok) {
            throw new Error(response.status === 404 ? "本が見つかりません。" : `書籍情報を取得できませんでした（HTTP ${response.status}）。`);
        }
        const book = await response.json();
        if (closed) return;
        document.getElementById("reader-title").textContent = book.title;
        document.title = `${book.title} - Dantalian 読書`;
        if (!book.epub_file_hash) throw new Error("この本には EPUB ファイルが登録されていません。「書籍情報」からファイルを登録してください。");
        const bookUrl = `/epubs/${encodeURIComponent(book.epub_file_hash)}`;
        download.href = bookUrl;
        download.download = book.epub_file_name || book.epub_file_hash;
        download.classList.remove("hidden");
        if (/\.pdf$/i.test(book.epub_file_name || "")) throw new Error("Bibi は PDF に対応していません。ファイルをダウンロードして閲覧してください。");
        if (!window["bibi:jo"]?.Bucket) throw new Error("読書リーダーを読み込めませんでした。Bibi の配布物が配置されているか確認してください。");

        const readerUrl = new URL("/vendor/bibi/", location.origin);
        Object.entries({
            book: bookUrl,
            preset: "dantalian",
            "book-key": `book:${book.id}`,
            "content-hash": book.epub_file_hash,
            "storage-namespace": "dantalian",
            "storage-user": "local",
        }).forEach(([key, value]) => readerUrl.searchParams.set(key, value));
        bucket = new window["bibi:jo"].Bucket({ "bibi-href": readerUrl.href });
        bucket.api.subscribe("bibi:position-changed", (event) => {
            if (session && event.sessionId === session.sessionId && event.bookKey === session.bookKey && event.contentHash === session.contentHash) {
                updateProgress(event.position);
            }
        });
        container.append(bucket.Anchor);
        bucket.embed();
        bucket.Frame.title = `${book.title}の本文`;
        bucket.Frame.style.width = "100%";
        bucket.Frame.style.height = "100%";
        await bucket.api.ready;
        if (closed) return;
        session = await bucket.api.getState();
        updateProgress(await bucket.api.getPosition());
        if (!closed) status.classList.add("hidden");
    } catch (error) {
        if (closed) return;
        dispose();
        status.classList.remove("hidden");
        status.setAttribute("role", "alert");
        message.textContent = `本を開けませんでした。${error.message}`;
    }
})();
