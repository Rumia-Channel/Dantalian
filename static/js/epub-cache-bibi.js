// Trusted host extension: retain Bibi's EPUB parser and supply device-local bytes.
Bibi.x({
    id: "DantalianEpubCache",
    description: "Bounded device EPUB cache",
    version: "1.0.0",
})(() => {
    const reader = window.dantalianEpubReader;
    const extract = O.loadZippedBookData;
    if (!reader?.descriptor || typeof extract !== "function") {
        throw new Error("EPUB キャッシュの読書コンテキストを読み込めませんでした。");
    }
    O.loadZippedBookData = async function loadCachedEpub(data) {
        if (typeof data !== "string" || data !== reader.descriptor.url) return extract(data);
        return reader.consume(async blob => {
            await extract(blob);
            return B.Type === "EPUB";
        });
    };
});
