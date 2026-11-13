// EPUB のローカルキャッシュ。
//
// EPUB 本体（Blob）と管理情報（版・容量・最終利用時刻）は、同じオリジンの
// IndexedDB にだけ保存する。サーバー側の DB・ファイル・S3 には何も保存しない。
// 容量上限と世代（generation）も IndexedDB に置き、localStorage へ写しを残さない。
// タブをまたぐ容量制限・世代チェック・LRU 削除は IndexedDB の readwrite
// トランザクションでまとめて行うため、同時に書き込んでも上限は超えない。
//
// 管理情報（entries）と本体（blobs）は別のストアに分ける。容量集計や削除は
// 管理情報だけを走査すれば足り、本体の Blob を読み出さずに済む。
(function initEpubCache() {
    "use strict";

    const DB_NAME = "dantalian-epub-cache-v1";
    const DB_VERSION = 1;
    const STORE_META = "meta";
    const STORE_ENTRIES = "entries";
    const STORE_BLOBS = "blobs";
    const INDEX_BY_LAST_USED = "by_last_used";
    const SETTINGS_KEY = "settings";
    const DEFAULT_LIMIT_BYTES = 512 * 1024 * 1024;

    let dbPromise = null;

    // ---------------------------------------------------------------------
    // 小さな補助
    // ---------------------------------------------------------------------

    function isBlobLike(value) {
        if (!value || typeof value !== "object") return false;
        if (typeof Blob !== "undefined" && value instanceof Blob) return true;
        return typeof value.size === "number" && typeof value.arrayBuffer === "function";
    }


    function entrySize(record) {
        return record && Number.isSafeInteger(record.size) && record.size > 0 ? record.size : 0;
    }

    function requireText(value, label) {
        if (typeof value !== "string" || !value) {
            throw new TypeError(`${label} が不正です`);
        }
        return value;
    }

    // revisionTime は null（更新されないファイルのハッシュ）か、
    // ミリ秒の数値（更新時刻）だけを受け付ける。
    // 解析できない時刻（NaN）は「不明」として扱い、本を捨てない。
    function normalizeRevisionTime(value) {
        if (value == null) return null;
        if (typeof value !== "number") {
            throw new TypeError("revisionTime が不正です");
        }
        if (Number.isNaN(value)) return null;
        if (!Number.isFinite(value) || value < 0) {
            throw new TypeError("revisionTime が不正です");
        }
        return value;
    }

    // 管理レコード（制御情報）。不正な値は既定値へ戻す。
    // clock は LRU の順序を保つ単調増加の時計。同じミリ秒やタブをまたいでも
    // 最終利用時刻が厳密に増えるように、記録と更新時刻はここから採番する。
    function readControlRecord(record) {
        const limitBytes = record && Number.isSafeInteger(record.limitBytes) && record.limitBytes >= 0
            ? record.limitBytes
            : DEFAULT_LIMIT_BYTES;
        const generation = record && Number.isSafeInteger(record.generation) && record.generation >= 1
            ? record.generation
            : 1;
        const clock = record && Number.isSafeInteger(record.clock) && record.clock >= 0
            ? record.clock
            : 0;
        return { limitBytes, generation, clock };
    }

    function writeControlRecord(meta, control) {
        meta.put({
            key: SETTINGS_KEY,
            limitBytes: control.limitBytes,
            generation: control.generation,
            clock: control.clock,
        });
    }

    // ---------------------------------------------------------------------
    // DB 接続
    // ---------------------------------------------------------------------

    function openDb() {
        if (typeof indexedDB === "undefined" || !indexedDB) {
            return Promise.reject(new Error("このブラウザは EPUB キャッシュに対応していません"));
        }
        if (dbPromise) return dbPromise;

        const promise = new Promise((resolve, reject) => {
            let settled = false;
            let request = null;
            try {
                request = indexedDB.open(DB_NAME, DB_VERSION);
            } catch (error) {
                settled = true;
                reject(error instanceof Error ? error : new Error("EPUB キャッシュDBを開けません"));
                return;
            }

            request.onupgradeneeded = () => {
                const db = request.result;
                if (!db.objectStoreNames.contains(STORE_META)) {
                    db.createObjectStore(STORE_META, { keyPath: "key" });
                }
                if (!db.objectStoreNames.contains(STORE_ENTRIES)) {
                    const entries = db.createObjectStore(STORE_ENTRIES, { keyPath: "id" });
                    entries.createIndex(INDEX_BY_LAST_USED, "lastUsed", { unique: false });
                }
                if (!db.objectStoreNames.contains(STORE_BLOBS)) {
                    db.createObjectStore(STORE_BLOBS);
                }
            };

            request.onsuccess = () => {
                const db = request.result;
                // 別タブが DB を更新したら接続を閉じ、次の操作で開き直せるようにする。
                db.onversionchange = () => {
                    db.close();
                    if (dbPromise === promise) dbPromise = null;
                };
                db.onclose = () => {
                    if (dbPromise === promise) dbPromise = null;
                };
                if (settled) {
                    db.close();
                    return;
                }
                settled = true;
                resolve(db);
            };

            // 古いバージョンの DB を別タブが開いている場合。閉じられたら再試行できる。
            request.onblocked = () => {
                if (settled) return;
                settled = true;
                if (dbPromise === promise) dbPromise = null;
                reject(new Error("EPUB キャッシュDBが別のタブで使用中です。時間をおいて再試行してください"));
            };

            request.onerror = () => {
                if (settled) return;
                settled = true;
                if (dbPromise === promise) dbPromise = null;
                reject(request.error || new Error("EPUB キャッシュDBを開けません"));
            };
        });

        // 開けなかった場合は接続を破棄して、次の呼び出しで再試行できるようにする。
        promise.catch(() => {
            if (dbPromise === promise) dbPromise = null;
        });

        dbPromise = promise;
        return promise;
    }

    // トランザクションはコールバックだけで進める。await を挟むと
    // トランザクションが inactive になるブラウザがあるため。
    function runTransaction(db, stores, mode, body) {
        return new Promise((resolve, reject) => {
            let transaction = null;
            try {
                transaction = db.transaction(stores, mode);
            } catch (error) {
                reject(error instanceof Error ? error : new Error("EPUB キャッシュの操作に失敗しました"));
                return;
            }

            let settled = false;
            let hasValue = false;
            let value;

            const fail = (error) => {
                if (settled) return;
                settled = true;
                reject(error instanceof Error ? error : new Error("EPUB キャッシュの操作に失敗しました"));
            };

            transaction.oncomplete = () => {
                if (settled) return;
                settled = true;
                if (hasValue) {
                    resolve(value);
                } else {
                    reject(new Error("EPUB キャッシュの処理が完了しませんでした"));
                }
            };
            transaction.onerror = (event) => {
                const source = event && event.target;
                fail((source && source.error) || transaction.error || new Error("EPUB キャッシュの操作に失敗しました"));
            };
            transaction.onabort = () => {
                fail(transaction.error || new Error("EPUB キャッシュの操作が中断されました"));
            };

            try {
                body(transaction, (result) => {
                    if (settled) return;
                    hasValue = true;
                    value = result;
                });
            } catch (error) {
                try {
                    transaction.abort();
                } catch {
                    // すでに終わっている場合は何もしない。
                }
                fail(error);
            }
        });
    }

    // 古いものから消して空きを作る。足りたら onDone を呼ぶ。
    // skipId は消さない対象（置き換えようとしている同じ版の記録）。
    function evictToFit(entries, blobs, limitBytes, skipId, projected, onDone) {
        let remaining = projected;
        let finished = false;
        const finish = () => {
            if (finished) return;
            finished = true;
            onDone(remaining);
        };
        const request = entries.index(INDEX_BY_LAST_USED).openCursor();
        request.onsuccess = () => {
            const cursor = request.result;
            if (!cursor) {
                finish();
                return;
            }
            if (remaining <= limitBytes) {
                finish();
                return;
            }
            const id = cursor.primaryKey;
            if (id === skipId) {
                cursor.continue();
                return;
            }
            const size = entrySize(cursor.value);
            cursor.delete();
            blobs.delete(id);
            remaining -= size;
            cursor.continue();
        };
    }

    // ---------------------------------------------------------------------
    // 公開 API
    // ---------------------------------------------------------------------

    // 現在の設定（容量上限と世代）を返す。
    function settings() {
        return openDb().then((db) => runTransaction(db, [STORE_META], "readonly", (transaction, done) => {
            const request = transaction.objectStore(STORE_META).get(SETTINGS_KEY);
            request.onsuccess = () => {
                const current = readControlRecord(request.result);
                done({ limitBytes: current.limitBytes, generation: current.generation });
            };
        }));
    }

    // 使用量と冊数。本体の Blob は読み出さずに管理情報だけ集計する。
    function stats() {
        return openDb().then((db) => runTransaction(db, [STORE_META, STORE_ENTRIES], "readonly", (transaction, done) => {
            const meta = transaction.objectStore(STORE_META);
            const entries = transaction.objectStore(STORE_ENTRIES);
            let limitBytes = DEFAULT_LIMIT_BYTES;

            const settingsRequest = meta.get(SETTINGS_KEY);
            settingsRequest.onsuccess = () => {
                limitBytes = readControlRecord(settingsRequest.result).limitBytes;
            };

            let usedBytes = 0;
            let count = 0;
            const cursorRequest = entries.openCursor();
            cursorRequest.onsuccess = () => {
                const cursor = cursorRequest.result;
                if (cursor) {
                    usedBytes += entrySize(cursor.value);
                    count += 1;
                    cursor.continue();
                    return;
                }
                done({ limitBytes, usedBytes, count });
            };
        }));
    }

    // 容量上限を変える。0 は「保存しない」意味。
    // 上限を変えると世代が進むので、取得中にあった古いダウンロードは捨てられる。
    function setLimitBytes(bytes) {
        if (!Number.isSafeInteger(bytes) || bytes < 0) {
            return Promise.reject(new RangeError("容量上限は 0 以上の整数で指定してください"));
        }
        return openDb()
            .then((db) => runTransaction(db, [STORE_META, STORE_ENTRIES, STORE_BLOBS], "readwrite", (transaction, done) => {
                const meta = transaction.objectStore(STORE_META);
                const entries = transaction.objectStore(STORE_ENTRIES);
                const blobs = transaction.objectStore(STORE_BLOBS);

                const request = meta.get(SETTINGS_KEY);
                request.onsuccess = () => {
                    const current = readControlRecord(request.result);
                    const changed = current.limitBytes !== bytes;
                    // 容量を変えると世代が進むので、取得中にあった古いダウンロードは捨てられる。
                    writeControlRecord(meta, {
                        limitBytes: bytes,
                        generation: changed ? current.generation + 1 : current.generation,
                        clock: current.clock,
                    });

                    // 容量を下げたときは古いものから消す（0 にすると全部消える）。
                    let usedBytes = 0;
                    const cursorRequest = entries.openCursor();
                    cursorRequest.onsuccess = () => {
                        const cursor = cursorRequest.result;
                        if (cursor) {
                            usedBytes += entrySize(cursor.value);
                            cursor.continue();
                            return;
                        }
                        if (usedBytes <= bytes) {
                            done(null);
                            return;
                        }
                        evictToFit(entries, blobs, bytes, null, usedBytes, () => done(null));
                    };
                };
            }))
            .then(() => stats());
    }

    // 保存済みの EPUB を全部消す。世代を進めて、取得中にあった書き戻しは捨てる。
    function clear() {
        return openDb()
            .then((db) => runTransaction(db, [STORE_META, STORE_ENTRIES, STORE_BLOBS], "readwrite", (transaction, done) => {
                const meta = transaction.objectStore(STORE_META);
                transaction.objectStore(STORE_ENTRIES).clear();
                transaction.objectStore(STORE_BLOBS).clear();
                const request = meta.get(SETTINGS_KEY);
                request.onsuccess = () => {
                    const current = readControlRecord(request.result);
                    // 全部消して世代を進める。取得中にあった書き戻しは捨てられる。
                    writeControlRecord(meta, {
                        limitBytes: current.limitBytes,
                        generation: current.generation + 1,
                        clock: current.clock,
                    });
                    done(null);
                };
            }))
            .then(() => stats());
    }

    // 最後に使った時刻を更新する。読み出し自体を失敗させないため、失敗は捨てる。
    // 管理レコードの単調増加時計から時刻を採番するので、同じミリ秒でも
    // タブをまたいでも最終利用時刻は厳密に増える（LRU の順序が壊れない）。
    function touchLastUsed(id) {
        return openDb()
            .then((db) => runTransaction(db, [STORE_META, STORE_ENTRIES], "readwrite", (transaction, done) => {
                const meta = transaction.objectStore(STORE_META);
                const entries = transaction.objectStore(STORE_ENTRIES);
                const controlRequest = meta.get(SETTINGS_KEY);
                controlRequest.onsuccess = () => {
                    const control = readControlRecord(controlRequest.result);
                    const request = entries.get(id);
                    request.onsuccess = () => {
                        if (request.result) {
                            const lastUsed = Math.max(Date.now(), control.clock + 1);
                            entries.put({ ...request.result, lastUsed });
                            writeControlRecord(meta, { ...control, clock: lastUsed });
                        }
                        done(null);
                    };
                };
            }));
    }

    // 指定した版が保存されていれば取り出す。版が違えば古い版は返さない。
    function read(key, revision) {
        if (typeof key !== "string" || !key) return Promise.resolve(null);
        if (typeof revision !== "string" || !revision) return Promise.resolve(null);
        const id = key;

        return openDb()
            .then((db) => runTransaction(db, [STORE_ENTRIES, STORE_BLOBS], "readonly", (transaction, done) => {
                const entries = transaction.objectStore(STORE_ENTRIES);
                const blobs = transaction.objectStore(STORE_BLOBS);
                const request = entries.get(id);
                request.onsuccess = () => {
                    const record = request.result;
                    if (!record || record.revision !== revision) {
                        done(null);
                        return;
                    }
                    const blobRequest = blobs.get(id);
                    blobRequest.onsuccess = () => {
                        const blob = blobRequest.result;
                        if (!isBlobLike(blob)) {
                            done(null);
                            return;
                        }
                        done({
                            key: record.key,
                            revision: record.revision,
                            revisionTime: record.revisionTime ?? null,
                            blob,
                            size: entrySize(record) || blob.size,
                        });
                    };
                };
            }))
            .then((result) => {
                if (!result) return null;
                // 最終利用時刻の更新まで済ませてから返す（順序が読取後に確定する）。
                // 更新に失敗しても読み出し自体は成功として扱う。
                return touchLastUsed(id).then(() => result, () => result);
            });
    }

    // 保存する。世代が古ければ保存しない（消したキャッシュを復活させない）。
    // 引数が不正なときは例外ではなく reject で返す（Promise として扱えるように）。
    async function put(entry, generation) {
        const key = requireText(entry?.key, "key");
        const revision = requireText(entry?.revision, "revision");
        const revisionTime = normalizeRevisionTime(entry?.revisionTime);
        const blob = entry?.blob;
        if (!isBlobLike(blob)) {
            throw new TypeError("blob が不正です");
        }
        if (!Number.isSafeInteger(blob.size) || blob.size <= 0) {
            throw new TypeError("blob が空です");
        }
        if (generation != null && !Number.isSafeInteger(generation)) {
            throw new TypeError("generation が不正です");
        }

        const id = key;
        return openDb()
            .then((db) => runTransaction(db, [STORE_META, STORE_ENTRIES, STORE_BLOBS], "readwrite", (transaction, done) => {
                const meta = transaction.objectStore(STORE_META);
                const entries = transaction.objectStore(STORE_ENTRIES);
                const blobs = transaction.objectStore(STORE_BLOBS);

                const settingsRequest = meta.get(SETTINGS_KEY);
                settingsRequest.onsuccess = () => {
                    const current = readControlRecord(settingsRequest.result);

                    if (generation != null && generation !== current.generation) {
                        // 取得中にキャッシュが消えた、または容量が変わった。
                        done({ stored: false, reason: "changed" });
                        return;
                    }
                    if (current.limitBytes === 0) {
                        done({ stored: false, reason: "disabled" });
                        return;
                    }
                    if (blob.size > current.limitBytes) {
                        done({ stored: false, reason: "oversize" });
                        return;
                    }

                    // 管理情報を走査して使用量と既存記録を求める（本体は読み出さない）。
                    let usedBytes = 0;
                    let existing = null;
                    const cursorRequest = entries.openCursor();
                    cursorRequest.onsuccess = () => {
                        const cursor = cursorRequest.result;
                        if (cursor) {
                            const record = cursor.value;
                            usedBytes += entrySize(record);
                            if (record.id === id) existing = record;
                            cursor.continue();
                            return;
                        }

                        // 新しい更新時刻より古い版で上書きしない。
                        if (existing
                            && typeof existing.revisionTime === "number"
                            && typeof revisionTime === "number"
                            && revisionTime < existing.revisionTime) {
                            done({ stored: false, reason: "superseded" });
                            return;
                        }

                        // 同じ版を置き換えるときは古い分を差し引く。
                        const projected = usedBytes - entrySize(existing) + blob.size;
                        const write = () => {
                            // 管理レコードの単調増加時計から採番する。タブをまたいでも
                            // 同じミリ秒でも、最終利用時刻は他の記録より必ず新しい。
                            const lastUsed = Math.max(Date.now(), current.clock + 1);
                            entries.put({
                                id,
                                key,
                                revision,
                                revisionTime,
                                size: blob.size,
                                lastUsed,
                            });
                            blobs.put(blob, id);
                            writeControlRecord(meta, {
                                limitBytes: current.limitBytes,
                                generation: current.generation,
                                clock: lastUsed,
                            });
                            done({ stored: true });
                        };

                        if (projected <= current.limitBytes) {
                            write();
                            return;
                        }
                        // 収まらない分は古いものから消す。
                        evictToFit(entries, blobs, current.limitBytes, id, projected, write);
                    };
                };
            }));
    }

    // 作品単位で一つの版だけを保存する。古い読書タブから新しい版を削除させない。
    function remove(key, revision) {
        if (typeof key !== "string" || !key) return Promise.resolve();
        return openDb()
            .then((db) => runTransaction(db, [STORE_ENTRIES, STORE_BLOBS], "readwrite", (transaction, done) => {
                const entries = transaction.objectStore(STORE_ENTRIES);
                const request = entries.get(key);
                request.onsuccess = () => {
                    if (request.result && (revision == null || request.result.revision === revision)) {
                        entries.delete(key);
                        transaction.objectStore(STORE_BLOBS).delete(key);
                    }
                    done(null);
                };
            }))
            .then(() => undefined);
    }

    const api = Object.freeze({
        DEFAULT_LIMIT_BYTES,
        settings,
        stats,
        setLimitBytes,
        clear,
        read,
        put,
        remove,
    });

    globalThis.epubCache = api;
})();
