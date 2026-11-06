// アップロード後、空欄の時間は自動保存し、既存値は確認後に上書きする。
// 非同期処理中に変更された時間や、対象外になった行は更新しない。

function secondsToMmSs(secs) {
    if (!isFinite(secs) || secs <= 0) return null;
    const total = Math.round(secs);
    const m = Math.floor(total / 60);
    const s = total % 60;
    return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

function probeAudioDuration(url) {
    return new Promise((resolve) => {
        const a = new Audio();
        a.preload = "metadata";
        a.src = url;
        const cleanup = () => {
            a.onloadedmetadata = null;
            a.onerror = null;
        };
        a.onloadedmetadata = () => {
            const d = a.duration;
            cleanup();
            resolve(d);
        };
        a.onerror = () => {
            cleanup();
            resolve(NaN);
        };
    });
}

function findTrackDurationInput(trackId) {
    const list = document.getElementById("edit-tracks-list");
    return list?.querySelector(`.edit-track-dur-input[data-track-id="${trackId}"]`);
}

async function offerToApplyAudioDuration(
    editType,
    parentId,
    trackId,
    fileHash,
    reloadFn,
    knownSeconds = null,
    operationToken = null,
) {
    const parsedKnownSeconds = Number(knownSeconds);
    const secs = Number.isFinite(parsedKnownSeconds) && parsedKnownSeconds > 0
        ? parsedKnownSeconds
        : await probeAudioDuration(`/audio/${fileHash}?_=${Date.now()}`);
    const formatted = secondsToMmSs(secs);
    if (!formatted) return false;
    // 長さの取得中に別の操作が始まっていたら、確認も反映もしない。
    if (operationToken !== null && !trackOperationIsCurrent(operationToken)) return false;

    // 長さの取得中に入力された値を尊重する。
    let input = findTrackDurationInput(trackId);
    if (!input) return false;
    const currentValue = input.value.trim();

    if (currentValue !== "") {
        const ok = await showConfirm({
            message: `音声ファイルの長さ ${formatted} をトラックの長さに設定しますか？`,
            okLabel: "設定する",
            cancelLabel: "設定しない",
            okClass: "btn btn-sm btn-outline-accent",
        });
        if (!ok) return false;
        if (operationToken !== null && !trackOperationIsCurrent(operationToken)) return false;
    }

    // 保存する直前に長さ欄を読み直す。確認待ちの間に別の編集が入って
    // いれば、その編集を上書きしない。
    input = findTrackDurationInput(trackId);
    if (!input) return false;
    if (input.value.trim() !== currentValue) return false;

    // 保存失敗時は再描画せず、入力内容を保持する。
    const saved = await saveTrackField(parentId, trackId, "duration", formatted, editType);
    if (!saved) return false;
    await reloadFn(parentId);
    return true;
}

// 既存のクライアント単体テストと同じ CommonJS 読み込み口。
if (typeof module !== "undefined" && module.exports) {
    module.exports = { offerToApplyAudioDuration };
}
