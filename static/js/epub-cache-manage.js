// ローカル EPUB キャッシュ（このブラウザ・この端末のみ）の設定 UI。
//
// サーバー設定 (/api/settings) とは完全に独立している。容量・使用状況・削除は
// すべて IndexedDB 上の epubCache モジュールだけを操作し、サーバー（Dantalian の
// DB・ファイル・S3）へ EPUB を保存したり容量設定を送信したりすることはない。
(() => {
    const BYTES_PER_MIB = 1024 * 1024;
    const DEFAULT_LIMIT_MIB = 512;
    const MAX_LIMIT_MIB = Math.floor(Number.MAX_SAFE_INTEGER / BYTES_PER_MIB);

    const state = {
        form: null,
        limitInput: null,
        usage: null,
        status: null,
        error: null,
        meter: null,
        buttons: [],
        limitDirty: false,
        busy: false,
        unavailable: false,
    };

    function cacheApi() {
        return window.epubCache || null;
    }

    function indexedDbAvailable() {
        try {
            return typeof indexedDB !== "undefined" && indexedDB !== null;
        } catch {
            return false;
        }
    }

    function unavailableError(message) {
        const error = new Error(message);
        error.unavailable = true;
        return error;
    }

    function describeError(error) {
        if (!error) return "原因不明のエラー";
        if (typeof error === "string") return error;
        return error.message || String(error);
    }

    function formatBytes(bytes) {
        if (!Number.isFinite(bytes) || bytes < 0) return null;
        if (bytes < BYTES_PER_MIB) return `${bytes.toLocaleString("ja-JP")} バイト`;
        const mib = bytes / BYTES_PER_MIB;
        if (mib < 1024) return `${mib.toFixed(1)} MiB`;
        return `${(mib / 1024).toFixed(2)} GiB`;
    }

    function limitMibText(bytes) {
        if (!Number.isFinite(bytes) || bytes < 0) return null;
        return String(Math.round(bytes / BYTES_PER_MIB));
    }

    function setStatus(text) {
        if (!state.status) return;
        state.status.textContent = text || "";
    }

    function setError(text) {
        if (!state.error) return;
        state.error.textContent = text || "";
    }

    function setUsage(text) {
        if (!state.usage) return;
        state.usage.textContent = text;
    }

    // 使用量メーター。上限や使用量が不明なときは非表示に戻す。
    function updateMeter(usedBytes, limitBytes) {
        const meter = state.meter;
        if (!meter) return;
        const fill = meter.querySelector(".epub-cache-meter-fill");
        if (!Number.isFinite(usedBytes) || !Number.isFinite(limitBytes) || limitBytes <= 0) {
            meter.hidden = true;
            return;
        }
        const ratio = Math.min(Math.max(usedBytes / limitBytes, 0), 1);
        meter.hidden = false;
        fill.style.width = `${(ratio * 100).toFixed(1)}%`;
        meter.dataset.level = ratio >= 0.95 ? "full" : ratio >= 0.75 ? "warn" : "ok";
    }

    function updateDisabledState() {
        const disabled = state.busy || state.unavailable;
        state.buttons.forEach(button => { button.disabled = disabled; });
        if (state.limitInput) state.limitInput.disabled = disabled;
    }

    // 使用状況の表示。取得できなかった値は 0 とせず「取得できません」と表示する。
    function applyStats(stats) {
        const limitBytes = Number(stats?.limitBytes);
        const usedBytes = Number(stats?.usedBytes);
        const count = Number(stats?.count);

        // 未保存の入力があれば上書きしない（状態更新・削除では容量入力を保つ）。
        if (!state.limitDirty) {
            const mib = limitMibText(limitBytes);
            if (mib !== null) state.limitInput.value = mib;
        }

        const parts = [];
        const usedText = formatBytes(usedBytes);
        if (usedText) {
            parts.push(`使用中 ${usedText}（${usedBytes.toLocaleString("ja-JP")} バイト）`);
        } else {
            parts.push("使用量は取得できません");
        }
        if (Number.isFinite(count) && count >= 0) {
            parts.push(`/ 保存数 ${count.toLocaleString("ja-JP")} 冊`);
        } else {
            parts.push("/ 保存数は取得できません");
        }
        const limitText = formatBytes(limitBytes);
        if (limitText) {
            parts.push(limitBytes === 0 ? `/ 上限 ${limitText}（キャッシュは無効）` : `/ 上限 ${limitText}`);
        }
        updateMeter(usedBytes, limitBytes);
        setUsage(parts.join(" "));
    }

    function applyUnknownUsage(clearLimit) {
        updateMeter(NaN, NaN);
        setUsage("使用量・保存数・上限を取得できませんでした。");
        if (clearLimit && !state.limitDirty && state.limitInput) {
            state.limitInput.value = "";
        }
    }

    async function readStats() {
        const api = cacheApi();
        if (!api) {
            throw unavailableError("EPUB キャッシュ機能を読み込めませんでした。ページを再読み込みしてください。");
        }
        if (!indexedDbAvailable()) {
            throw unavailableError("このブラウザは IndexedDB を利用できないため、EPUB キャッシュを使用できません。");
        }
        if (typeof api.stats !== "function") {
            throw unavailableError("EPUB キャッシュ機能が想定どおり動作しません。");
        }
        const stats = await api.stats();
        state.unavailable = false;
        return stats;
    }

    // 使用状況の読み込み。失敗時はエラー表示だけを行い、例外は投げない
    // （保存処理の成否と使用状況の表示を混ぜないため）。
    async function loadStats() {
        try {
            const stats = await readStats();
            applyStats(stats);
            setStatus("");
            setError("");
            return stats;
        } catch (error) {
            state.unavailable = Boolean(error?.unavailable);
            applyUnknownUsage(state.unavailable);
            setError(`EPUB キャッシュの状態を取得できませんでした: ${describeError(error)}`);
            return null;
        }
    }

    async function refreshStatus() {
        if (state.busy) return null;
        state.busy = true;
        updateDisabledState();
        try {
            return await loadStats();
        } finally {
            state.busy = false;
            updateDisabledState();
        }
    }

    async function saveLimit(event) {
        event.preventDefault();
        if (state.busy) return;
        const raw = state.limitInput.value.trim();
        const mib = Number(raw);
        if (raw === "" || !Number.isFinite(mib) || !Number.isInteger(mib) || mib < 0 || mib > MAX_LIMIT_MIB) {
            setError("容量は 0 以上の整数（MiB）で入力してください。");
            state.limitInput.focus();
            return;
        }
        const bytes = mib * BYTES_PER_MIB;
        if (!Number.isSafeInteger(bytes)) {
            setError("容量の値が大きすぎます。小さい値を指定してください。");
            state.limitInput.focus();
            return;
        }

        state.busy = true;
        updateDisabledState();
        setStatus("保存中…");
        setError("");
        try {
            const api = cacheApi();
            if (!api || typeof api.setLimitBytes !== "function") {
                throw unavailableError("EPUB キャッシュ機能を読み込めませんでした。ページを再読み込みしてください。");
            }
            if (!indexedDbAvailable()) {
                throw unavailableError("このブラウザは IndexedDB を利用できないため、EPUB キャッシュを使用できません。");
            }
            const stats = await api.setLimitBytes(bytes);
            state.unavailable = false;
            state.limitDirty = false;
            if (stats) {
                applyStats(stats);
            } else {
                await loadStats();
            }
            if (mib === 0) {
                const emptied = stats && Number(stats.count) === 0;
                setStatus(emptied
                    ? "保存しました。EPUB キャッシュを無効化し、このブラウザに保存済みの EPUB を削除しました。"
                    : "保存しました。EPUB キャッシュを無効化しました。");
            } else {
                setStatus("保存しました。");
            }
        } catch (error) {
            state.unavailable = Boolean(error?.unavailable);
            setStatus("");
            setError(`容量を保存できませんでした: ${describeError(error)}`);
        } finally {
            state.busy = false;
            updateDisabledState();
        }
    }

    async function confirmClear() {
        const message = "このブラウザに保存された EPUB キャッシュをすべて削除します。よろしいですか？";
        if (typeof showConfirm === "function") {
            return await showConfirm({
                message,
                okLabel: "削除する",
                cancelLabel: "キャンセル",
                okClass: "btn btn-sm btn-outline-danger",
            });
        }
        return window.confirm(message);
    }

    async function clearCache() {
        if (state.busy) return;
        const confirmed = await confirmClear();
        if (!confirmed) return;

        state.busy = true;
        updateDisabledState();
        setStatus("削除中…");
        setError("");
        try {
            const api = cacheApi();
            if (!api || typeof api.clear !== "function") {
                throw unavailableError("EPUB キャッシュ機能を読み込めませんでした。ページを再読み込みしてください。");
            }
            if (!indexedDbAvailable()) {
                throw unavailableError("このブラウザは IndexedDB を利用できないため、EPUB キャッシュを使用できません。");
            }
            const stats = await api.clear();
            state.unavailable = false;
            // 削除では容量（上限）自体は変わらない。未保存の入力も上書きしない。
            if (stats) {
                applyStats(stats);
            } else {
                await loadStats();
            }
            setStatus("このブラウザに保存された EPUB キャッシュを削除しました。");
        } catch (error) {
            state.unavailable = Boolean(error?.unavailable);
            setStatus("");
            setError(`EPUB キャッシュを削除できませんでした: ${describeError(error)}`);
        } finally {
            state.busy = false;
            updateDisabledState();
        }
    }

    function buildForm(container) {
        container.textContent = "";
        const form = document.createElement("form");
        form.className = "settings-form-section";
        form.id = "epub-cache-form";
        form.noValidate = true;
        form.innerHTML = `
            <p class="settings-note">この設定とキャッシュ本体は、このブラウザ・この端末（ブラウザプロファイル）の中だけに保存されます。サーバー（Dantalian の DB・ファイル・S3）へ EPUB を保存することはなく、容量設定もサーバーへは送信されません。</p>
            <div class="settings-form-row">
                <label class="settings-form-label-inline" for="epub-cache-limit-mib">キャッシュ容量 (MiB)</label>
                <input type="number" id="epub-cache-limit-mib" class="form-input" min="0" step="1" inputmode="numeric" placeholder="${DEFAULT_LIMIT_MIB}" style="width:8rem" aria-describedby="epub-cache-limit-hint">
            </div>
            <p id="epub-cache-limit-hint" class="settings-note settings-note--tight">0 を指定すると、このブラウザの EPUB キャッシュを無効にし、保存済みの EPUB も削除します。</p>
            <div class="settings-form-row">
                <span class="settings-form-label-inline">使用状況</span>
                <span class="epub-cache-meter" id="epub-cache-meter" hidden>
                    <span class="epub-cache-meter-fill"></span>
                </span>
                <span id="epub-cache-usage" class="settings-label epub-cache-usage-text">取得中…</span>
            </div>
            <div class="settings-form-group">
                <button type="submit" class="btn btn-primary">保存</button>
                <button type="button" class="btn btn-ghost" id="epub-cache-refresh">状態を更新</button>
                <button type="button" class="btn btn-outline-danger" id="epub-cache-clear">キャッシュした EPUB を削除</button>
            </div>
            <p class="settings-note">共有端末では、EPUB はこのブラウザプロファイルに残ります。ブラウザの容量制限などで自動削除される場合もあります。利用時には書誌情報と認証をオンラインで確認するため、オフライン閲覧機能ではありません。narou.rs の書誌情報や EPUB を Dantalian サーバーへ取り込むことはありません。</p>
            <div id="epub-cache-status" class="settings-msg settings-msg--ok" role="status" aria-live="polite"></div>
            <div id="epub-cache-error" class="settings-msg settings-msg--error" role="alert"></div>`;
        container.append(form);

        state.form = form;
        state.limitInput = form.querySelector("#epub-cache-limit-mib");
        state.usage = form.querySelector("#epub-cache-usage");
        state.status = form.querySelector("#epub-cache-status");
        state.error = form.querySelector("#epub-cache-error");
        state.meter = form.querySelector("#epub-cache-meter");
        state.buttons = Array.from(form.querySelectorAll("button"));

        form.addEventListener("submit", saveLimit);
        form.querySelector("#epub-cache-refresh").addEventListener("click", () => { void refreshStatus(); });
        form.querySelector("#epub-cache-clear").addEventListener("click", () => { void clearCache(); });
        state.limitInput.addEventListener("input", () => { state.limitDirty = true; });
    }

    // 設定タブを開いたときに使用状況を更新する（未保存の容量入力は上書きしない）。
    function observePanel() {
        const panel = document.getElementById("panel-settings");
        if (!panel || typeof MutationObserver === "undefined") return;
        const observer = new MutationObserver(() => {
            if (panel.classList.contains("hidden")) return;
            void refreshStatus();
        });
        observer.observe(panel, { attributes: true, attributeFilter: ["class", "aria-hidden"] });
    }

    async function renderEpubCacheSettings() {
        const container = document.getElementById("epub-cache-settings");
        if (!container) return;
        if (!state.form) buildForm(container);
        await refreshStatus();
    }

    window.renderEpubCacheSettings = renderEpubCacheSettings;
    window.epubCacheManage = {
        render: renderEpubCacheSettings,
        refresh: refreshStatus,
        clear: clearCache,
    };

    observePanel();
    void renderEpubCacheSettings();
})();
