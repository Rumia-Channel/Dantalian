(() => {
    let loading = false;
    window.renderNarouSettings = async function () {
        const container = document.getElementById("narou-settings");
        if (loading || container.querySelector("form")) return;
        loading = true;
        container.textContent = "設定を読み込み中…";
        try {
            const settings = await request("/api/settings");
            const tags = JSON.parse(settings["narou.tags"] || "[]");
            const source = (settings["narou.url"] || "").trim();
            container.innerHTML = `
                <form class="settings-form-section">
                    <div class="settings-form-row">
                        <label class="settings-form-label-inline" for="narou-url">narou.rs URL</label>
                        <input id="narou-url" class="form-input" type="url" placeholder="http://localhost:8000" value="${escapeAttr(source)}">
                    </div>
                    <div class="settings-form-row">
                        <a id="narou-open" class="btn btn-ghost" href="${escapeAttr(source)}" target="_blank" rel="noopener noreferrer">narou.rs を開く（ログイン）</a>
                    </div>
                    <p>ブラウザが narou.rs へ直接アクセスします。narou.rs には利用者自身のログイン（Cloudflare Access セッションなど）が使われ、アクセス権のトークンをブラウザへ配る必要はありません。</p>
                    <div class="settings-form-row">
                        <label class="settings-form-label"><input id="narou-favorites" type="checkbox" ${settings["narou.use_favorites"] !== "false" ? "checked" : ""}> narou.rs のお気に入りタグを対象にする</label>
                    </div>
                    <div class="settings-form-row">
                        <label class="settings-form-label-inline" for="narou-tags">追加の対象タグ<br>（1 行に 1 タグ）</label>
                        <textarea id="narou-tags" class="form-input" rows="5">${escapeHtml(tags.join("\n"))}</textarea>
                    </div>
                    <p>対象は指定タグとお気に入りタグの和集合に完全一致する小説です。画面にはブラウザが取得した書誌情報だけを表示し、カバーは表示されません。接続先・対象タグ以外は保存しません。</p>
                    <div class="settings-form-group">
                        <button class="btn btn-primary" type="submit">設定を保存</button>
                        <button class="btn btn-ghost" id="narou-check" type="button">接続を確認</button>
                        <a class="btn btn-ghost" href="/books/">本棚を開く</a>
                    </div>
                </form>
                <div id="narou-result" role="status" aria-live="polite"></div>`;
            const form = container.querySelector("form");
            const result = document.getElementById("narou-result");
            const urlInput = document.getElementById("narou-url");
            const openLink = document.getElementById("narou-open");

            // 「narou.rs を開く」リンクは入力中の接続先へ追従させる。
            const syncOpenLink = () => {
                const value = urlInput.value.trim();
                openLink.href = value;
                openLink.hidden = !value;
            };
            syncOpenLink();
            urlInput.addEventListener("input", syncOpenLink);

            // 接続確認は narou.rs へブラウザが直接取りに行く（Dantalian を経由しない）。
            async function checkConnection() {
                const client = window.narouClient;
                if (!client) {
                    result.textContent = "narou クライアントを読み込めませんでした。";
                    return;
                }
                result.textContent = "接続を確認中…（narou.rs から直接取得します）";
                try {
                    const catalogue = await client.loadCatalogue({ verifyConnection: true });
                    if (catalogue.state === "disabled") {
                        result.textContent = "narou.rs の接続先が未設定です。URL を保存してください。";
                        return;
                    }
                    const tags = catalogue.tags.join("、") || "なし";
                    if (!catalogue.items.length) {
                        result.textContent = `接続できました（対象タグ: ${tags}）。該当する Web 小説はありません。`;
                        return;
                    }
                    result.textContent = `接続できました。Web 小説 ${catalogue.items.length} 件（対象タグ: ${tags}）を narou.rs から直接取得します。`;
                } catch (error) {
                    result.textContent = `接続を確認できませんでした: ${error.message}`;
                    const source = client.openUrl(error.connection);
                    if (source) {
                        const anchor = document.createElement("a");
                        anchor.href = source;
                        anchor.target = "_blank";
                        anchor.rel = "noopener noreferrer";
                        anchor.textContent = "narou.rs でログイン";
                        result.append(" ", anchor);
                    }
                }
            }

            async function saveSettings() {
                if (!form.reportValidity()) return;
                const controls = Array.from(form.elements);
                const values = {
                    "narou.url": urlInput.value.trim(),
                    "narou.tags": JSON.stringify([...new Set(document.getElementById("narou-tags").value.split(/\r?\n/).map(tag => tag.trim()).filter(Boolean))]),
                    "narou.use_favorites": String(document.getElementById("narou-favorites").checked),
                };
                controls.forEach(control => { control.disabled = true; });
                result.textContent = "保存中…";
                try {
                    await request("/api/settings", "PUT", values);
                    syncOpenLink();
                    await checkConnection();
                } catch (error) {
                    result.textContent = error.message;
                } finally {
                    controls.forEach(control => { control.disabled = false; });
                }
            }

            form.addEventListener("submit", event => { event.preventDefault(); void saveSettings(); });
            document.getElementById("narou-check").addEventListener("click", () => { void checkConnection(); });
        } catch (error) {
            container.textContent = error.message;
            const retry = document.createElement("button");
            retry.className = "btn btn-ghost";
            retry.textContent = "再読み込み";
            retry.addEventListener("click", window.renderNarouSettings);
            container.append(retry);
        } finally {
            loading = false;
        }
    };

    async function request(url, method = "GET", body) {
        const response = await fetch(url, {
            method,
            cache: "no-store",
            ...(body === undefined ? {} : { headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) }),
        });
        const payload = await response.json();
        if (!response.ok) throw new Error(payload.error || `HTTP ${response.status}`);
        return payload;
    }
})();
