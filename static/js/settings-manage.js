const SETTINGS_KEYS = [
    "discogs_token",
    "upload.cover_max_mb",
    "upload.audio_max_mb",
    "upload.file_max_mb",
    "backup.enabled",
    "backup.schedule_time",
    "backup.schedule_tz",
    "backup.retention",
    "backup.dest_type",
    "backup.local_path",
    "backup.webdav_url",
    "backup.webdav_user",
    "backup.webdav_pass",
    "backup.s3_endpoint",
    "backup.s3_region",
    "backup.s3_bucket",
    "backup.s3_access_key",
    "backup.s3_secret_key",
    "backup.s3_prefix",
    "media_sync.enabled",
    "media_sync.types",
    "media_sync.schedule_time",
    "media_sync.schedule_tz",
    "media_sync.s3_endpoint",
    "media_sync.s3_region",
    "media_sync.s3_bucket",
    "media_sync.s3_access_key",
    "media_sync.s3_secret_key",
    "media_sync.s3_prefix",
];

const TZ_OPTIONS = [
    { value: "Asia/Tokyo", label: "Asia/Tokyo (UTC+9)" },
    { value: "Asia/Seoul", label: "Asia/Seoul (UTC+9)" },
    { value: "Asia/Shanghai", label: "Asia/Shanghai (UTC+8)" },
    { value: "Asia/Hong_Kong", label: "Asia/Hong_Kong (UTC+8)" },
    { value: "Asia/Bangkok", label: "Asia/Bangkok (UTC+7)" },
    { value: "Asia/Kolkata", label: "Asia/Kolkata (UTC+5:30)" },
    { value: "Europe/London", label: "Europe/London" },
    { value: "Europe/Berlin", label: "Europe/Berlin (UTC+1/+2)" },
    { value: "America/New_York", label: "America/New_York (UTC-5/-4)" },
    { value: "America/Chicago", label: "America/Chicago (UTC-6/-5)" },
    { value: "America/Denver", label: "America/Denver (UTC-7/-6)" },
    { value: "America/Los_Angeles", label: "America/Los_Angeles (UTC-8/-7)" },
    { value: "UTC", label: "UTC" },
];

async function loadSettings() {
    try {
        const res = await fetch("/api/settings");
        if (!res.ok) return {};
        return await res.json();
    } catch {
        return {};
    }
}

async function saveSettings(settings) {
    const body = {};
    for (const key of SETTINGS_KEYS) {
        if (settings[key] !== undefined) {
            body[key] = String(settings[key]);
        }
    }
    try {
        const res = await fetch("/api/settings", {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
        });
        if (res.ok) {
            alert("設定を保存しました。");
            return true;
        }
    } catch {}
    alert("設定の保存に失敗しました。");
    return false;
}

function getValue(settings, key, defaultVal) {
    if (settings[key] !== undefined && settings[key] !== null && settings[key] !== "") {
        return settings[key];
    }
    return defaultVal !== undefined ? defaultVal : "";
}

async function renderSettingsForm() {
    const settings = await loadSettings();
    const container = document.getElementById("settings-form");
    if (!container) return;

    const enabled = getValue(settings, "backup.enabled", "false");
    const scheduleTime = getValue(settings, "backup.schedule_time", "");
    const scheduleTz = getValue(settings, "backup.schedule_tz", "Asia/Tokyo");
    const retention = getValue(settings, "backup.retention", "7");
    const destType = getValue(settings, "backup.dest_type", "local");

    const msEnabled = getValue(settings, "media_sync.enabled", "false");
    const msTypesStr = getValue(settings, "media_sync.types", "epubs,audio");
    const msScheduleTime = getValue(settings, "media_sync.schedule_time", "");
    const msScheduleTz = getValue(settings, "media_sync.schedule_tz", "Asia/Tokyo");
    const msTypesSet = new Set(
        msTypesStr.split(",").map((s) => s.trim().toLowerCase()).filter(Boolean)
    );

    const tzOptionsHtml = TZ_OPTIONS.map((tz) =>
        `<option value="${escapeAttr(tz.value)}" ${tz.value === scheduleTz ? "selected" : ""}>${escapeHtml(tz.label)}</option>`
    ).join("");

    const msTzOptionsHtml = TZ_OPTIONS.map((tz) =>
        `<option value="${escapeAttr(tz.value)}" ${tz.value === msScheduleTz ? "selected" : ""}>${escapeHtml(tz.label)}</option>`
    ).join("");

    const destOptions = [
        { value: "local", label: "ローカル" },
        { value: "webdav", label: "WebDAV" },
        { value: "s3", label: "S3互換" },
    ];
    const destOptionsHtml = destOptions.map((d) =>
        `<option value="${escapeAttr(d.value)}" ${d.value === destType ? "selected" : ""}>${escapeHtml(d.label)}</option>`
    ).join("");

    const uploadCoverMb = getValue(settings, "upload.cover_max_mb", "10");
    const uploadAudioMb = getValue(settings, "upload.audio_max_mb", "100");
    const uploadFileMb = getValue(settings, "upload.file_max_mb", "500");

    container.innerHTML = `
        <div class="settings-form-section">
            <h3>外部API</h3>

            <div class="settings-form-row">
                <label class="settings-form-label-inline" for="s-discogs-token">Discogs Token</label>
                <input type="password" id="s-discogs-token" value="${escapeAttr(getValue(settings, "discogs_token", ""))}" class="form-input" placeholder="Discogs Personal Access Token">
                <span class="settings-label" style="font-size:0.72rem;color:var(--color-text-dim);margin-left:0.5rem">CD検索のMusicBrainzフォールバックに使用</span>
            </div>
        </div>

        <div class="settings-form-section">
            <h3>アップロード上限</h3>
            <div class="settings-form-row">
                <label class="settings-form-label-inline" for="s-upload-cover-mb">カバー画像 (MB)</label>
                <input type="number" id="s-upload-cover-mb" value="${escapeAttr(uploadCoverMb)}" min="1" max="4096" class="form-input" style="width:6rem">
            </div>
            <div class="settings-form-row">
                <label class="settings-form-label-inline" for="s-upload-audio-mb">音声ファイル (MB)</label>
                <input type="number" id="s-upload-audio-mb" value="${escapeAttr(uploadAudioMb)}" min="1" max="4096" class="form-input" style="width:6rem">
            </div>
            <div class="settings-form-row">
                <label class="settings-form-label-inline" for="s-upload-file-mb">書籍ファイル epub/pdf/zip (MB)</label>
                <input type="number" id="s-upload-file-mb" value="${escapeAttr(uploadFileMb)}" min="1" max="4096" class="form-input" style="width:6rem">
            </div>
            <div class="settings-form-row">
                <span class="settings-label" style="font-size:0.72rem;color:var(--color-text-dim);">アプリ側の上限です。前面のリバースプロキシ (nginx: client_max_body_size 等) や Cloudflare の上限もこれ以上に引き上げる必要があります。上限は 4096MB (4GB) まで。</span>
            </div>
        </div>

        <div class="settings-form-section">
            <h3>バックアップ</h3>

            <div class="settings-form-row">
                <label class="settings-form-label">
                    <input type="checkbox" id="s-backup-enabled" ${enabled === "true" ? "checked" : ""}>
                    バックアップを有効にする
                </label>
            </div>

            <div class="settings-form-row">
                <label class="settings-form-label-inline">定時バックアップ時刻</label>
                <div class="settings-form-group">
                    <input type="time" id="s-schedule-time" value="${escapeAttr(scheduleTime)}" class="form-input" style="width:auto">
                    <span class="settings-label">at</span>
                    <select id="s-schedule-tz" class="form-input" style="width:auto">${tzOptionsHtml}</select>
                </div>
            </div>

            <div class="settings-form-row">
                <label class="settings-form-label-inline" for="s-retention">保持世代数</label>
                <input type="number" id="s-retention" value="${escapeAttr(retention)}" min="1" max="365" class="form-input" style="width:5rem">
            </div>

            <div class="settings-form-row">
                <label class="settings-form-label-inline" for="s-dest-type">バックアップ先</label>
                <select id="s-dest-type" class="form-input" style="width:auto">${destOptionsHtml}</select>
            </div>

            <div id="dest-fields">${renderDestFields(destType, settings)}</div>
        </div>

        <div class="settings-form-section">
            <h3>メディア同期 (S3 upload-only)</h3>

            <div class="settings-form-row">
                <label class="settings-form-label">
                    <input type="checkbox" id="s-media-sync-enabled" ${msEnabled === "true" ? "checked" : ""}>
                    メディア同期を有効にする
                </label>
            </div>

            <div class="settings-form-row">
                <label class="settings-form-label-inline">対象メディア</label>
                <div class="settings-form-group">
                    <label class="settings-form-label"><input type="checkbox" id="s-media-sync-type-images" ${msTypesSet.has("images") ? "checked" : ""}> 画像 (cover)</label>
                    <label class="settings-form-label"><input type="checkbox" id="s-media-sync-type-audio" ${msTypesSet.has("audio") ? "checked" : ""}> 音声 (audio)</label>
                    <label class="settings-form-label"><input type="checkbox" id="s-media-sync-type-epubs" ${msTypesSet.has("epubs") ? "checked" : ""}> 書籍ファイル (epub/pdf/zip)</label>
                </div>
            </div>

            <div class="settings-form-row">
                <label class="settings-form-label-inline">定時同期時刻</label>
                <div class="settings-form-group">
                    <input type="time" id="s-media-sync-schedule-time" value="${escapeAttr(msScheduleTime)}" class="form-input" style="width:auto">
                    <span class="settings-label">at</span>
                    <select id="s-media-sync-schedule-tz" class="form-input" style="width:auto">${msTzOptionsHtml}</select>
                </div>
            </div>

            <div class="settings-form-row">
                <span class="settings-label" style="font-size:0.78rem;color:var(--color-text-dim)">
                    S3 設定が空の場合はバックアップの S3 設定を流用します。
                </span>
            </div>

            <div class="settings-form-row">
                <label class="settings-form-label-inline" for="s-media-sync-s3-endpoint">エンドポイント</label>
                <input type="url" id="s-media-sync-s3-endpoint" value="${escapeAttr(getValue(settings, "media_sync.s3_endpoint", ""))}" class="form-input" placeholder="https://s3.example.com (未入力ならバックアップの S3 設定を使用)">
            </div>
            <div class="settings-form-row">
                <label class="settings-form-label-inline" for="s-media-sync-s3-region">リージョン</label>
                <input type="text" id="s-media-sync-s3-region" value="${escapeAttr(getValue(settings, "media_sync.s3_region", ""))}" class="form-input" placeholder="未入力ならバックアップの S3 設定 / us-east-1">
            </div>
            <div class="settings-form-row">
                <label class="settings-form-label-inline" for="s-media-sync-s3-bucket">バケット</label>
                <input type="text" id="s-media-sync-s3-bucket" value="${escapeAttr(getValue(settings, "media_sync.s3_bucket", ""))}" class="form-input" placeholder="未入力ならバックアップの S3 設定を使用">
            </div>
            <div class="settings-form-row">
                <label class="settings-form-label-inline" for="s-media-sync-s3-access-key">アクセスキー</label>
                <input type="text" id="s-media-sync-s3-access-key" value="${escapeAttr(getValue(settings, "media_sync.s3_access_key", ""))}" class="form-input">
            </div>
            <div class="settings-form-row">
                <label class="settings-form-label-inline" for="s-media-sync-s3-secret-key">シークレットキー</label>
                <input type="password" id="s-media-sync-s3-secret-key" value="${escapeAttr(getValue(settings, "media_sync.s3_secret_key", ""))}" class="form-input">
            </div>
            <div class="settings-form-row">
                <label class="settings-form-label-inline" for="s-media-sync-s3-prefix">プレフィックス</label>
                <input type="text" id="s-media-sync-s3-prefix" value="${escapeAttr(getValue(settings, "media_sync.s3_prefix", ""))}" class="form-input" placeholder="dantalian (未入力ならバックアップの S3 設定を使用)">
            </div>

            <div class="settings-form-row">
                <button class="btn btn-secondary" type="button" onclick="runMediaSyncNow()">今すぐ同期</button>
                <span class="settings-label" style="font-size:0.75rem;color:var(--color-text-dim)">※ upload-only。S3 側の削除は行いません。</span>
            </div>

            <div id="media-sync-result"></div>
        </div>

        <div class="settings-form-row">
            <button class="btn btn-primary" onclick="submitSettings()">保存</button>
        </div>
    `;

    document.getElementById("s-dest-type").addEventListener("change", function () {
        const fields = document.getElementById("dest-fields");
        fields.innerHTML = renderDestFields(this.value, settings);
    });
}

function renderDestFields(destType, settings) {
    switch (destType) {
        case "local":
            return `
                <div class="settings-form-row">
                    <label class="settings-form-label-inline" for="s-local-path">保存ディレクトリ</label>
                    <input type="text" id="s-local-path" value="${escapeAttr(getValue(settings, "backup.local_path", ""))}" class="form-input" placeholder="/path/to/backups">
                </div>`;
        case "webdav":
            return `
                <div class="settings-form-row">
                    <label class="settings-form-label-inline" for="s-webdav-url">URL</label>
                    <input type="url" id="s-webdav-url" value="${escapeAttr(getValue(settings, "backup.webdav_url", ""))}" class="form-input" placeholder="https://webdav.example.com/backups/">
                </div>
                <div class="settings-form-row">
                    <label class="settings-form-label-inline" for="s-webdav-user">ユーザー名</label>
                    <input type="text" id="s-webdav-user" value="${escapeAttr(getValue(settings, "backup.webdav_user", ""))}" class="form-input">
                </div>
                <div class="settings-form-row">
                    <label class="settings-form-label-inline" for="s-webdav-pass">パスワード</label>
                    <input type="password" id="s-webdav-pass" value="${escapeAttr(getValue(settings, "backup.webdav_pass", ""))}" class="form-input">
                </div>`;
        case "s3":
            return `
                <div class="settings-form-row">
                    <label class="settings-form-label-inline" for="s-s3-endpoint">エンドポイント</label>
                    <input type="url" id="s-s3-endpoint" value="${escapeAttr(getValue(settings, "backup.s3_endpoint", ""))}" class="form-input" placeholder="https://s3.example.com">
                </div>
                <div class="settings-form-row">
                    <label class="settings-form-label-inline" for="s-s3-region">リージョン</label>
                    <input type="text" id="s-s3-region" value="${escapeAttr(getValue(settings, "backup.s3_region", "us-east-1"))}" class="form-input">
                </div>
                <div class="settings-form-row">
                    <label class="settings-form-label-inline" for="s-s3-bucket">バケット</label>
                    <input type="text" id="s-s3-bucket" value="${escapeAttr(getValue(settings, "backup.s3_bucket", ""))}" class="form-input">
                </div>
                <div class="settings-form-row">
                    <label class="settings-form-label-inline" for="s-s3-access-key">アクセスキー</label>
                    <input type="text" id="s-s3-access-key" value="${escapeAttr(getValue(settings, "backup.s3_access_key", ""))}" class="form-input">
                </div>
                <div class="settings-form-row">
                    <label class="settings-form-label-inline" for="s-s3-secret-key">シークレットキー</label>
                    <input type="password" id="s-s3-secret-key" value="${escapeAttr(getValue(settings, "backup.s3_secret_key", ""))}" class="form-input">
                </div>
                <div class="settings-form-row">
                    <label class="settings-form-label-inline" for="s-s3-prefix">プレフィックス</label>
                    <input type="text" id="s-s3-prefix" value="${escapeAttr(getValue(settings, "backup.s3_prefix", ""))}" class="form-input" placeholder="dantalian/">
                </div>`;
        default:
            return "";
    }
}

async function submitSettings() {
    const settings = {};
    settings["discogs_token"] = document.getElementById("s-discogs-token").value;
    settings["upload.cover_max_mb"] = document.getElementById("s-upload-cover-mb").value;
    settings["upload.audio_max_mb"] = document.getElementById("s-upload-audio-mb").value;
    settings["upload.file_max_mb"] = document.getElementById("s-upload-file-mb").value;
    settings["backup.enabled"] = document.getElementById("s-backup-enabled").checked ? "true" : "false";
    settings["backup.schedule_time"] = document.getElementById("s-schedule-time").value;
    settings["backup.schedule_tz"] = document.getElementById("s-schedule-tz").value;
    settings["backup.retention"] = document.getElementById("s-retention").value;
    const destType = document.getElementById("s-dest-type").value;
    settings["backup.dest_type"] = destType;

    switch (destType) {
        case "local":
            settings["backup.local_path"] = document.getElementById("s-local-path").value;
            break;
        case "webdav":
            settings["backup.webdav_url"] = document.getElementById("s-webdav-url").value;
            settings["backup.webdav_user"] = document.getElementById("s-webdav-user").value;
            settings["backup.webdav_pass"] = document.getElementById("s-webdav-pass").value;
            break;
        case "s3":
            settings["backup.s3_endpoint"] = document.getElementById("s-s3-endpoint").value;
            settings["backup.s3_region"] = document.getElementById("s-s3-region").value;
            settings["backup.s3_bucket"] = document.getElementById("s-s3-bucket").value;
            settings["backup.s3_access_key"] = document.getElementById("s-s3-access-key").value;
            settings["backup.s3_secret_key"] = document.getElementById("s-s3-secret-key").value;
            settings["backup.s3_prefix"] = document.getElementById("s-s3-prefix").value;
            break;
    }

    settings["media_sync.enabled"] = document.getElementById("s-media-sync-enabled").checked ? "true" : "false";
    const msTypes = [];
    if (document.getElementById("s-media-sync-type-images").checked) msTypes.push("images");
    if (document.getElementById("s-media-sync-type-audio").checked) msTypes.push("audio");
    if (document.getElementById("s-media-sync-type-epubs").checked) msTypes.push("epubs");
    if (msTypes.length === 0) {
        alert("メディア同期の「対象メディア」が1つも選択されていません。最低1つをチェックしてください。");
        return;
    }
    settings["media_sync.types"] = msTypes.join(",");
    settings["media_sync.schedule_time"] = document.getElementById("s-media-sync-schedule-time").value;
    settings["media_sync.schedule_tz"] = document.getElementById("s-media-sync-schedule-tz").value;
    settings["media_sync.s3_endpoint"] = document.getElementById("s-media-sync-s3-endpoint").value;
    settings["media_sync.s3_region"] = document.getElementById("s-media-sync-s3-region").value;
    settings["media_sync.s3_bucket"] = document.getElementById("s-media-sync-s3-bucket").value;
    settings["media_sync.s3_access_key"] = document.getElementById("s-media-sync-s3-access-key").value;
    settings["media_sync.s3_secret_key"] = document.getElementById("s-media-sync-s3-secret-key").value;
    settings["media_sync.s3_prefix"] = document.getElementById("s-media-sync-s3-prefix").value;

    await saveSettings(settings);
}

function renderMediaSyncResult(container, data, isError) {
    if (!data) {
        container.innerHTML = "";
        return;
    }
    const lines = [];
    if (isError) {
        lines.push(`<div style="color:var(--color-danger,#c0392b);font-size:0.85rem;">エラー: ${escapeHtml(data.error || "unknown")}</div>`);
    } else {
        const failed = data.failed ?? 0;
        const ok = data.ok === true && failed === 0;
        const headerColor = ok ? "var(--color-text-secondary)" : "var(--color-danger,#c0392b)";
        const headerText = ok ? "同期完了" : (failed > 0 ? `同期失敗 (${failed}件)` : "同期に失敗しました");
        const message = data.message ? ` — ${escapeHtml(String(data.message))}` : "";
        lines.push(`<div style="font-size:0.85rem;color:${headerColor};">${headerText}: scanned=${data.scanned ?? 0}, uploaded=${data.uploaded ?? 0}, skipped=${data.skipped ?? 0}, failed=${failed}, missing_local=${data.missing_local ?? 0}${message}</div>`);
        if (data.per_type && typeof data.per_type === "object") {
            const rows = Object.keys(data.per_type).map((k) => {
                const r = data.per_type[k];
                return `<li>${escapeHtml(k)}: scanned=${r.scanned ?? 0}, uploaded=${r.uploaded ?? 0}, skipped=${r.skipped ?? 0}, failed=${r.failed ?? 0}, missing_local=${r.missing_local ?? 0}</li>`;
            }).join("");
            if (rows) lines.push(`<ul style="margin:0.25rem 0 0 1rem;font-size:0.8rem;">${rows}</ul>`);
        }
    }
    container.innerHTML = lines.join("");
}

async function runMediaSyncNow() {
    const container = document.getElementById("media-sync-result");
    if (container) container.innerHTML = '<div style="font-size:0.85rem;color:var(--color-text-dim);">同期中...</div>';
    try {
        const res = await fetch("/api/media-sync/run", { method: "POST" });
        let body = null;
        try { body = await res.json(); } catch {}
        if (!res.ok) {
            renderMediaSyncResult(container, body || { error: `HTTP ${res.status}` }, true);
            return;
        }
        renderMediaSyncResult(container, body, false);
    } catch (e) {
        renderMediaSyncResult(container, { error: String(e) }, true);
    }
}
