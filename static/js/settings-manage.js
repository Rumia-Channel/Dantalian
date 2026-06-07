const SETTINGS_KEYS = [
    "discogs_token",
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

    const tzOptionsHtml = TZ_OPTIONS.map((tz) =>
        `<option value="${escapeAttr(tz.value)}" ${tz.value === scheduleTz ? "selected" : ""}>${escapeHtml(tz.label)}</option>`
    ).join("");

    const destOptions = [
        { value: "local", label: "ローカル" },
        { value: "webdav", label: "WebDAV" },
        { value: "s3", label: "S3互換" },
    ];
    const destOptionsHtml = destOptions.map((d) =>
        `<option value="${escapeAttr(d.value)}" ${d.value === destType ? "selected" : ""}>${escapeHtml(d.label)}</option>`
    ).join("");

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

            <div class="settings-form-row">
                <button class="btn btn-primary" onclick="submitSettings()">保存</button>
            </div>
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

    await saveSettings(settings);
}
