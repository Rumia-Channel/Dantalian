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
            container.innerHTML = `
                <form class="settings-form-section">
                    <div class="settings-form-row">
                        <label class="settings-form-label-inline" for="narou-url">narou.rs URL</label>
                        <input id="narou-url" class="form-input" type="url" required placeholder="http://localhost:8000" value="${escapeAttr(settings["narou.url"] || "")}">
                    </div>
                    <div class="settings-form-row">
                        <label class="settings-form-label-inline" for="narou-token">Bearer / Native トークン</label>
                        <input id="narou-token" class="form-input" type="password" autocomplete="new-password" placeholder="${settings["narou.token.__configured"] === "true" ? "設定済み（空欄なら維持）" : "認証が必要な場合のみ入力"}">
                    </div>
                    <div class="settings-form-row">
                        <label class="settings-form-label"><input id="narou-clear-token" type="checkbox"> 保存済みトークンを削除</label>
                    </div>
                    <p>Native 版は <code>.narou/ui_token</code>、Worker 版は Bearer token を指定します。Cloudflare Access 配下では下の Service Token も設定できます。</p>
                    <div class="settings-form-row">
                        <label class="settings-form-label-inline" for="narou-cf-id">CF Access Client ID</label>
                        <input id="narou-cf-id" class="form-input" type="password" autocomplete="new-password" placeholder="${settings["narou.cf_access_client_id.__configured"] === "true" ? "設定済み（空欄なら維持）" : "Service Token の Client ID"}">
                    </div>
                    <div class="settings-form-row">
                        <label class="settings-form-label-inline" for="narou-cf-secret">CF Access Client Secret</label>
                        <input id="narou-cf-secret" class="form-input" type="password" autocomplete="new-password" placeholder="${settings["narou.cf_access_client_secret.__configured"] === "true" ? "設定済み（空欄なら維持）" : "Service Token の Client Secret"}">
                    </div>
                    <div class="settings-form-row">
                        <label class="settings-form-label"><input id="narou-clear-cf" type="checkbox"> 保存済み Cloudflare Access 認証を削除</label>
                    </div>
                    <p>Bearer のみ・Cloudflare Access のみ・両方の併用に対応。Access 側のポリシーは Service Auth を使用してください。認証情報は HTTPS で送信します（localhost のみ HTTP 可）。</p>
                    <div class="settings-form-row">
                        <label class="settings-form-label"><input id="narou-favorites" type="checkbox" ${settings["narou.use_favorites"] !== "false" ? "checked" : ""}> narou.rs のお気に入りタグを対象にする</label>
                    </div>
                    <div class="settings-form-row">
                        <label class="settings-form-label-inline" for="narou-tags">追加の対象タグ<br>（1 行に 1 タグ）</label>
                        <textarea id="narou-tags" class="form-input" rows="5">${escapeHtml(tags.join("\n"))}</textarea>
                    </div>
                    <p>接続先には Dantalian サーバーからアクセスします。本棚を開く時と表示中の 5 分ごとに同期し、変更された EPUB だけ更新します。対象外・削除済みの本は非表示にします（読書状態は保持）。トップの一覧には表示しません。</p>
                    <div class="settings-form-group">
                        <button class="btn btn-primary" type="submit">設定を保存</button>
                        <button class="btn btn-primary" id="narou-sync" type="button">保存して今すぐ同期</button>
                        <a class="btn btn-ghost" href="/books/">本棚を開く</a>
                    </div>
                </form>
                <div id="narou-result" role="status" aria-live="polite"></div>`;
            const form = container.querySelector("form");
            const result = document.getElementById("narou-result");
            async function saveAndSync(sync) {
                if (!form.reportValidity()) return;
                const controls = Array.from(form.elements);
                const values = {
                    "narou.url": document.getElementById("narou-url").value.trim(),
                    "narou.tags": JSON.stringify([...new Set(document.getElementById("narou-tags").value.split(/\r?\n/).map(tag => tag.trim()).filter(Boolean))]),
                    "narou.use_favorites": String(document.getElementById("narou-favorites").checked),
                };
                const secrets = [
                    ["narou.token", "narou-token", "narou-clear-token"],
                    ["narou.cf_access_client_id", "narou-cf-id", "narou-clear-cf"],
                    ["narou.cf_access_client_secret", "narou-cf-secret", "narou-clear-cf"],
                ];
                for (const [key, inputId, clearId] of secrets) {
                    const input = document.getElementById(inputId);
                    if (document.getElementById(clearId).checked) values[key] = "";
                    else if (input.value) values[key] = input.value;
                }
                controls.forEach(control => { control.disabled = true; });
                result.textContent = sync ? "設定を保存して同期中…（EPUB の生成に時間がかかる場合があります）" : "保存中…";
                try {
                    const saved = await request("/api/settings", "PUT", values);
                    for (const [key, inputId, clearId] of secrets) {
                        const input = document.getElementById(inputId);
                        input.value = "";
                        input.placeholder = saved[`${key}.__configured`] === "true" ? "設定済み（空欄なら維持）" : "未設定";
                        document.getElementById(clearId).checked = false;
                    }
                    if (!sync) {
                        result.textContent = "設定を保存しました。";
                        return;
                    }
                    const report = await request("/api/narou/sync", "POST");
                    result.textContent = `対象 ${report.matched} 件：新規 ${report.imported} 件、更新 ${report.updated} 件、変更なし ${report.unchanged} 件、失敗 ${report.failed.length} 件。対象タグ：${report.tags.join("、") || "なし"}`;
                    if (report.failed.length) {
                        const list = document.createElement("ul");
                        for (const failure of report.failed) {
                            const item = document.createElement("li");
                            item.textContent = `${failure.title}：${failure.error}`;
                            list.append(item);
                        }
                        result.append(list);
                    }
                } catch (error) {
                    result.textContent = error.message;
                } finally {
                    controls.forEach(control => { control.disabled = false; });
                }
            }
            form.addEventListener("submit", event => { event.preventDefault(); void saveAndSync(false); });
            document.getElementById("narou-sync").addEventListener("click", () => { void saveAndSync(true); });
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
