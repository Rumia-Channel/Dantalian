// 空欄の自動設定、上書き確認、非同期処理中の編集保護を検証する。
import test from "node:test";
import assert from "node:assert/strict";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const {
  offerToApplyAudioDuration,
} = require("../../static/js/edit/track-duration.js");

const GLOBAL_KEYS = [
  "document",
  "showConfirm",
  "saveTrackField",
  "trackOperationIsCurrent",
  "Audio",
];

function snapshotGlobals() {
  const saved = {};
  for (const key of GLOBAL_KEYS) saved[key] = globalThis[key];
  return saved;
}

function restoreGlobals(saved) {
  for (const key of GLOBAL_KEYS) {
    if (saved[key] === undefined) delete globalThis[key];
    else globalThis[key] = saved[key];
  }
}

function makeInput(trackId, value) {
  return {
    value,
    dataset: { trackId: String(trackId) },
  };
}

// 対象トラックの入力欄だけを返す最小限の DOM。
function installDom(inputs) {
  const list = {
    querySelector(selector) {
      const trackId = /data-track-id="(\d+)"/.exec(selector)?.[1];
      return inputs.find((input) => input.dataset.trackId === trackId) ?? null;
    },
  };
  globalThis.document = {
    getElementById(id) {
      return id === "edit-tracks-list" ? list : null;
    },
  };
}

// 差し替えたグローバルを元に戻す (テスト間で漏らさない)。
function setup({ inputs = [], confirmResult = true, confirmCalls = null, saveResult = true, saveCalls = null, current = true }) {
  const saved = snapshotGlobals();
  installDom(inputs);
  globalThis.showConfirm = async (opts) => {
    if (confirmCalls) confirmCalls.push(opts);
    return confirmResult;
  };
  globalThis.saveTrackField = async (...args) => {
    if (saveCalls) saveCalls.push(args);
    return saveResult;
  };
  globalThis.trackOperationIsCurrent = () => current;
  globalThis.Audio = class {
    constructor() {
      this.duration = 95;
      this.onloadedmetadata = null;
      this.onerror = null;
    }
    set src(value) {
      this._src = value;
      setTimeout(() => {
        if (this.onloadedmetadata) this.onloadedmetadata();
      }, 0);
    }
    get src() {
      return this._src;
    }
  };
  return saved;
}

// --- 空欄なら自動保存 ----------------------------------------------

test("長さ欄が空なら確認なしで自動保存し、保存後に一覧を再描画する", async () => {
  const confirmCalls = [];
  const saveCalls = [];
  const reloadCalls = [];
  const saved = setup({ inputs: [makeInput(7, "")], confirmCalls, saveCalls });
  try {
    const result = await offerToApplyAudioDuration(
      "cd", 3, 7, "hash", async () => { reloadCalls.push(1); }, 125, null,
    );
    assert.equal(result, true);
    assert.equal(confirmCalls.length, 0, "空欄のときは確認ダイアログを出さない");
    assert.deepEqual(saveCalls, [[3, 7, "duration", "02:05", "cd"]]);
    assert.equal(reloadCalls.length, 1, "保存に成功したら再描画する");
  } finally {
    restoreGlobals(saved);
  }
});

test("空白だけの長さ欄も空欄扱いで自動保存する", async () => {
  const confirmCalls = [];
  const saveCalls = [];
  const saved = setup({ inputs: [makeInput(7, "   ")], confirmCalls, saveCalls });
  try {
    const result = await offerToApplyAudioDuration(
      "cd", 3, 7, "hash", async () => {}, 125, null,
    );
    assert.equal(result, true);
    assert.equal(confirmCalls.length, 0);
    assert.equal(saveCalls.length, 1);
  } finally {
    restoreGlobals(saved);
  }
});

// --- 既存値には上書き確認 -------------------------------------------

test("既に長さが入っているなら上書き確認を出し、取り消したら値は残す", async () => {
  const confirmCalls = [];
  const saveCalls = [];
  const saved = setup({ inputs: [makeInput(7, "03:00")], confirmResult: false, confirmCalls, saveCalls });
  try {
    const result = await offerToApplyAudioDuration(
      "cd", 3, 7, "hash", async () => {}, 125, null,
    );
    assert.equal(result, false);
    assert.equal(confirmCalls.length, 1, "上書き前に必ず確認する");
    assert.equal(saveCalls.length, 0, "取り消したら保存しない");
  } finally {
    restoreGlobals(saved);
  }
});

test("上書き確認で承認したら保存する", async () => {
  const confirmCalls = [];
  const saveCalls = [];
  const saved = setup({ inputs: [makeInput(7, "03:00")], confirmResult: true, confirmCalls, saveCalls });
  try {
    const result = await offerToApplyAudioDuration(
      "cd", 3, 7, "hash", async () => {}, 125, null,
    );
    assert.equal(result, true);
    assert.equal(confirmCalls.length, 1);
    assert.deepEqual(saveCalls, [[3, 7, "duration", "02:05", "cd"]]);
  } finally {
    restoreGlobals(saved);
  }
});

// --- 競合・失敗の扱い ------------------------------------------------

test("確認ダイアログの表示中に別編集が入ったら上書きしない", async () => {
  const saveCalls = [];
  const input = makeInput(7, "03:00");
  const saved = setup({ inputs: [input], confirmResult: true, saveCalls });
  try {
    // 確認ダイアログが開いている間にユーザーが長さを編集した状況を再現する。
    globalThis.showConfirm = async () => {
      input.value = "09:30";
      return true;
    };
    const result = await offerToApplyAudioDuration(
      "cd", 3, 7, "hash", async () => {}, 125, null,
    );
    assert.equal(result, false);
    assert.equal(saveCalls.length, 0, "別の編集を上書きしない");
    assert.equal(input.value, "09:30");
  } finally {
    restoreGlobals(saved);
  }
});

test("保存に失敗したら一覧を再描画しない", async () => {
  const reloadCalls = [];
  const saved = setup({ inputs: [makeInput(7, "")], saveResult: false });
  try {
    const result = await offerToApplyAudioDuration(
      "cd", 3, 7, "hash", async () => { reloadCalls.push(1); }, 125, null,
    );
    assert.equal(result, false);
    assert.equal(reloadCalls.length, 0, "保存失敗で再描画しない");
  } finally {
    restoreGlobals(saved);
  }
});

test("対象行が無ければ何も書き込まない", async () => {
  const confirmCalls = [];
  const saveCalls = [];
  const saved = setup({ inputs: [], confirmCalls, saveCalls });
  try {
    const result = await offerToApplyAudioDuration(
      "cd", 3, 7, "hash", async () => {}, 125, null,
    );
    assert.equal(result, false);
    assert.equal(confirmCalls.length, 0);
    assert.equal(saveCalls.length, 0);
  } finally {
    restoreGlobals(saved);
  }
});

test("破棄済みの操作 (古いトークン) なら保存しない", async () => {
  const saveCalls = [];
  const saved = setup({ inputs: [makeInput(7, "")], saveCalls, current: false });
  try {
    const result = await offerToApplyAudioDuration(
      "cd", 3, 7, "hash", async () => {}, 125, 42,
    );
    assert.equal(result, false);
    assert.equal(saveCalls.length, 0);
  } finally {
    restoreGlobals(saved);
  }
});


test("既知の長さが無ければ音声のメタデータから長さを取る", async () => {
  const saveCalls = [];
  const saved = setup({ inputs: [makeInput(7, "")], saveCalls });
  try {
    const result = await offerToApplyAudioDuration(
      "cd", 3, 7, "hash", async () => {}, null, null,
    );
    assert.equal(result, true);
    assert.deepEqual(saveCalls, [[3, 7, "duration", "01:35", "cd"]]);
  } finally {
    restoreGlobals(saved);
  }
});

test("音声の長さを取得できない場合は既存の時間を変更しない", async () => {
  const input = makeInput(7, "09:30");
  const confirmCalls = [];
  const saveCalls = [];
  const saved = setup({ inputs: [input], confirmCalls, saveCalls });
  globalThis.Audio = class {
    set src(value) {
      setTimeout(() => this.onerror(), 0);
    }
  };
  try {
    const result = await offerToApplyAudioDuration(
      "cd", 3, 7, "missing", async () => {}, null, null,
    );
    assert.equal(result, false);
    assert.equal(input.value, "09:30");
    assert.equal(confirmCalls.length, 0);
    assert.equal(saveCalls.length, 0);
  } finally {
    restoreGlobals(saved);
  }
});

test("長さの取得中に時間が入力された場合は確認なしで上書きしない", async () => {
  const input = makeInput(7, "");
  const confirmCalls = [];
  const saveCalls = [];
  const saved = setup({ inputs: [input], confirmResult: false, confirmCalls, saveCalls });
  globalThis.Audio = class {
    set src(value) {
      setTimeout(() => {
        input.value = "09:30";
        this.duration = 125;
        this.onloadedmetadata();
      }, 0);
    }
  };
  try {
    const result = await offerToApplyAudioDuration(
      "cd", 3, 7, "hash", async () => {}, null, null,
    );
    assert.equal(result, false);
    assert.equal(input.value, "09:30");
    assert.equal(confirmCalls.length, 1);
    assert.equal(saveCalls.length, 0);
  } finally {
    restoreGlobals(saved);
  }
});
