(async function openStandaloneReader() {
    const parameters = new URLSearchParams(location.search);
    const bookId = parameters.get("book");
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

    try {
        if (!bookId || !/^[1-9]\d*$/.test(bookId)) throw new Error("読書する本が指定されていません。");
        edit.href = `/edit/?mode=book&book=${encodeURIComponent(bookId)}`;
        edit.classList.remove("hidden");
        const response = await fetch(`/api/books/${encodeURIComponent(bookId)}`, { cache: "no-store" });
        if (!response.ok) {
            throw new Error(response.status === 404 ? "本が見つかりません。" : `書籍情報を取得できませんでした（HTTP ${response.status}）。`);
        }
        const book = await response.json();
        document.getElementById("reader-title").textContent = book.title;
        if (!book.epub_file_hash) throw new Error("この本には EPUB ファイルが登録されていません。「書籍情報」からファイルを登録してください。");
        const bookUrl = `/epubs/${encodeURIComponent(book.epub_file_hash)}`;
        download.href = bookUrl;
        download.download = book.epub_file_name || book.epub_file_hash;
        download.classList.remove("hidden");
        if (/\.pdf$/i.test(book.epub_file_name || "")) throw new Error("Bibi は PDF に対応していません。ファイルをダウンロードして閲覧してください。");

        const readerUrl = new URL("/vendor/bibi/dantalian.html", location.origin);
        const readerParameters = new URLSearchParams({
            book: bookUrl,
            "book-key": `book:${book.id}`,
            "content-hash": book.epub_file_hash,
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
    }
})();
