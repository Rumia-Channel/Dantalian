Bibi.preset({
    'extensions': window.dantalianEpubReader?.descriptor ? [
        {
            'src': `../extensions/extractor/at-once.js?v=${encodeURIComponent(window.dantalianEpubReader.readerVersion)}`,
            '-extract-': ['*']
        },
        { 'src': `/js/epub-cache-bibi.js?v=${window.dantalianEpubReader.version}` }
    ] : [],
    'extract-if-necessary': ['*'],
    'autostart': 'yes',
    'accept-pdf': 'yes',
    'resume-from-last-position': 'yes',
    'keep-settings': 'yes',
    'item-padding-top': 48,
    'item-padding-bottom': 48,
    'item-padding-left': 24,
    'item-padding-right': 24,
    'use-menubar': 'yes',
    'use-full-height': 'yes',
    'use-arrows': 'yes',
    'use-keys': 'yes',
    'use-slider': 'yes',
    'flipper-width': 1 / 3,
    'use-nombre': 'yes',
    'use-host-navigation': true,
    'host-navigation-api': '/api/reader-navigation',
    'host-navigation-paths': ['/', '/books/', '/edit/', '/reader/'],
    'allow-scripts-in-content': false,
    'request-with-credentials': (() => {
        try {
            const book = new URL(new URLSearchParams(location.search).get('book'), location.href);
            return book.origin === location.origin &&
                /^\/api\/narou\/novels\/(0|[1-9]\d*)\/epub$/.test(book.pathname);
        } catch {
            return false;
        }
    })()
});
