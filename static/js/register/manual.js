// --- Manual registration ---
let manualAllAuthors = [];
let manualAllSeries = [];
let manualAllGrandSeries = [];
let manualCoverFile = null;
let manualCoverPreview = null;
let manualAuthorIds = [];
let manualRendered = false;
let manualAuthorSelect = null;
let manualSeriesSelect = null;
let manualGrandSeriesSelect = null;

async function renderManualForm() {
    if (manualRendered) return;
    manualRendered = true;

    try {
        const [authorsRes, seriesRes, gsRes] = await Promise.all([
            fetch("/api/authors"),
            fetch("/api/series"),
            fetch("/api/grand-series"),
        ]);
        if (authorsRes.ok) manualAllAuthors = await authorsRes.json();
        if (seriesRes.ok) manualAllSeries = await seriesRes.json();
        if (gsRes.ok) manualAllGrandSeries = await gsRes.json();
    } catch {}

    const container = document.getElementById("manual-form-container");
    container.innerHTML = `
        <form class="edit-form" id="manual-form" onsubmit="submitManualBook(event)">
            <div class="edit-field">
                <label>登録種別 <span class="edit-required">*</span></label>
                <select name="media_type" id="manual-media-type">
                    <option value="book">書籍 (Book)</option>
                    <option value="cd">CD</option>
                </select>
            </div>
            <div id="manual-book-fields">
            <input type="hidden" name="series_id" value="">
            <input type="hidden" name="grand_series_id" value="">
            <div class="edit-field">
                <label>タイトル <span class="edit-required">*</span></label>
                <input type="text" name="title" required>
            </div>
            <div class="edit-field">
                <label>タイトル(よみ)</label>
                <input type="text" name="title_transcription">
            </div>
            <div class="edit-row">
                <div class="edit-field">
                    <label>出版社 / サークル名</label>
                    <input type="text" name="publisher">
                </div>
                <div class="edit-field">
                    <label>出版日 / 発行日</label>
                    <input type="text" name="publish_date">
                </div>
            </div>
            <div class="edit-row">
                <div class="edit-field">
                    <label>価格</label>
                    <input type="text" name="price">
                </div>
                <div class="edit-field">
                    <label>ページ数 / 体裁</label>
                    <input type="text" name="extent">
                </div>
            </div>
            <div class="edit-row">
                <div class="edit-field">
                    <label>巻</label>
                    <input type="text" name="volume">
                </div>
                <div class="edit-field">
                    <label>巻(よみ)</label>
                    <input type="text" name="volume_transcription">
                </div>
            </div>
            <div class="edit-field">
                <label>シリーズ名(NDL)</label>
                <input type="text" name="series_title">
            </div>
            <div class="edit-field">
                <label>シリーズ名(よみ)</label>
                <input type="text" name="series_title_transcription">
            </div>
            <div class="edit-field">
                <label>別タイトル</label>
                <input type="text" name="alternative">
            </div>
            <div class="edit-field">
                <label>別タイトル(よみ)</label>
                <input type="text" name="alternative_transcription">
            </div>
            <div class="edit-field">
                <label>説明</label>
                <textarea name="description" rows="6"></textarea>
            </div>
            <div class="edit-section">
                <h3 class="edit-section-title">ISBN / NDL 固有</h3>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>ISBN</label>
                        <input type="text" name="isbn">
                    </div>
                    <div class="edit-field">
                        <label>JPNO</label>
                        <input type="text" name="jpno">
                    </div>
                </div>
                <div class="edit-field">
                    <label>NDL URL</label>
                    <input type="text" name="ndl_url">
                </div>
            </div>
            <div class="edit-section">
                <h3 class="edit-section-title">ISDN 固有</h3>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>ISDN</label>
                        <input type="text" name="isdn">
                    </div>
                    <div class="edit-field">
                        <label>Cコード</label>
                        <input type="text" name="isdn_c_code">
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>区分</label>
                        <input type="text" name="isdn_class">
                    </div>
                    <div class="edit-field">
                        <label>形態</label>
                        <input type="text" name="isdn_type">
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>レーティング(性別)</label>
                        <input type="text" name="isdn_rating_gender">
                    </div>
                    <div class="edit-field">
                        <label>レーティング(年齢)</label>
                        <input type="text" name="isdn_rating_age">
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>地域</label>
                        <input type="text" name="isdn_region">
                    </div>
                    <div class="edit-field">
                        <label>ジャンルコード</label>
                        <input type="text" name="isdn_genre_code">
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>ジャンル名</label>
                        <input type="text" name="isdn_genre_name">
                    </div>
                    <div class="edit-field">
                        <label>ジャンル補足</label>
                        <input type="text" name="isdn_genre_user">
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>販売対象</label>
                        <input type="text" name="isdn_author">
                    </div>
                    <div class="edit-field">
                        <label>書籍形態(Cコード)</label>
                        <input type="text" name="isdn_shape">
                    </div>
                </div>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>内容(Cコード)</label>
                        <input type="text" name="isdn_contents">
                    </div>
                    <div class="edit-field">
                        <label>バーコード2段目</label>
                        <input type="text" name="isdn_barcode2">
                    </div>
                </div>
                <div class="edit-field">
                    <label>サンプル画像URL</label>
                    <input type="text" name="isdn_sample_image_url">
                </div>
            </div>
            <div class="edit-field">
                <label>表紙画像</label>
                <div class="manual-cover-row">
                    <label class="btn btn-xs btn-outline-success manual-cover-label">
                        ファイルを選択
                        <input type="file" id="manual-cover-input" accept="image/*" hidden>
                    </label>
                    <span class="manual-cover-filename" id="manual-cover-filename"></span>
                    ${manualCoverPreview ? `<img class="manual-cover-preview" id="manual-cover-preview" src="${manualCoverPreview}" alt="">` : '<img class="manual-cover-preview" id="manual-cover-preview" src="" alt="" hidden>'}
                </div>
            </div>
            <div class="edit-section">
                <h3 class="edit-section-title">アーティスト</h3>
                <div class="edit-author-list" id="manual-author-list"></div>
                <div class="edit-author-add">
                    <div id="manual-author-select-container"></div>
                    <button type="button" class="btn btn-xs btn-outline-success" onclick="addManualAuthor()">追加</button>
                </div>
            </div>
            <div class="edit-section">
                <h3 class="edit-section-title">シリーズ設定</h3>
                <div class="edit-row">
                    <div class="edit-field">
                        <label>シリーズ</label>
                        <div id="manual-series-select-container"></div>
                    </div>
                    <div class="edit-field">
                        <label>シリーズ巻数</label>
                        <input type="number" name="series_number" min="1" step="1">
                    </div>
                </div>
                <div class="edit-field">
                    <label>大シリーズ</label>
                    <div id="manual-grand-series-select-container"></div>
                </div>
            </div>
            <div id="manual-register-status"></div>
            <div class="edit-actions">
                <button type="submit" class="btn btn-md btn-primary">登録</button>
            </div>
            </div>
            <div id="manual-cd-fields" hidden>
            <div class="edit-field">
                <label>タイトル <span class="edit-required">*</span></label>
                <input type="text" name="cd_title" id="manual-cd-title">
            </div>
            <div class="edit-row">
                <div class="edit-field">
                    <label>アーティスト</label>
                    <input type="text" name="cd_artist">
                </div>
                <div class="edit-field">
                    <label>レーベル</label>
                    <input type="text" name="cd_label">
                </div>
            </div>
            <div class="edit-row">
                <div class="edit-field">
                    <label>カタログ番号</label>
                    <input type="text" name="cd_catalog_number">
                </div>
                <div class="edit-field">
                    <label>発売日</label>
                    <input type="text" name="cd_publish_date">
                </div>
            </div>
            <div class="edit-row">
                <div class="edit-field">
                    <label>ディスク数</label>
                    <input type="number" name="cd_disc_count" min="1" step="1">
                </div>
                <div class="edit-field">
                    <label>JAN</label>
                    <input type="text" name="cd_jan" id="manual-cd-jan">
                </div>
            </div>
            <div class="edit-field">
                <label>説明</label>
                <textarea name="cd_description" rows="4"></textarea>
            </div>
            <div class="edit-field">
                <label>表紙画像</label>
                <div class="manual-cover-row">
                    <label class="btn btn-xs btn-outline-success manual-cover-label">
                        ファイルを選択
                        <input type="file" id="manual-cd-cover-input" accept="image/*" hidden>
                    </label>
                    <span class="manual-cover-filename" id="manual-cd-cover-filename"></span>
                    <img class="manual-cover-preview" id="manual-cd-cover-preview" src="" alt="" hidden>
                </div>
            </div>
            <div id="manual-cd-register-status"></div>
            <div class="edit-actions">
                <button type="button" class="btn btn-md btn-primary" onclick="submitManualCd(event)">登録</button>
            </div>
            </div>
        </form>
    `;

    document.getElementById("manual-media-type").addEventListener("change", function () {
        const bookFields = document.getElementById("manual-book-fields");
        const cdFields = document.getElementById("manual-cd-fields");
        if (this.value === "cd") {
            bookFields.hidden = true;
            cdFields.hidden = false;
        } else {
            bookFields.hidden = false;
            cdFields.hidden = true;
        }
    });

    const form = document.getElementById("manual-form");

    manualSeriesSelect = createSearchableSelect(document.getElementById("manual-series-select-container"), {
        options: manualAllSeries.map((s) => ({ value: s.id, label: s.name })),
        value: null,
        placeholder: "なし",
        onChange: (val) => {
            form.querySelector("input[name=series_id]").value = val != null ? val : "";
        },
    });

    manualGrandSeriesSelect = createSearchableSelect(document.getElementById("manual-grand-series-select-container"), {
        options: manualAllGrandSeries.map((gs) => ({ value: gs.id, label: gs.name })),
        value: null,
        placeholder: "なし",
        onChange: (val) => {
            form.querySelector("input[name=grand_series_id]").value = val != null ? val : "";
        },
    });

    manualAuthorSelect = createSearchableSelect(document.getElementById("manual-author-select-container"), {
        options: manualAllAuthors.map((a) => ({ value: a.id, label: a.name })),
        value: null,
        placeholder: "アーティストを追加...",
        clearable: false,
    });

    document.getElementById("manual-form").querySelector("input[name=isbn]").addEventListener("input", function () {
        this.value = this.value
            .replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xFEE0))
            .replace(/[\s\u3000\-－ー]/g, "");
    });

    const isdnField = document.getElementById("manual-form").querySelector("input[name=isdn]");
    if (isdnField) {
        isdnField.addEventListener("input", function () {
            this.value = this.value
                .replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xFEE0))
                .replace(/[\s\u3000\-－ー]/g, "");
        });
    }

    document.getElementById("manual-cover-input").addEventListener("change", (e) => {
        const file = e.target.files[0];
        if (!file) return;
        manualCoverFile = file;
        document.getElementById("manual-cover-filename").textContent = file.name;
        const reader = new FileReader();
        reader.onload = (ev) => {
            const img = document.getElementById("manual-cover-preview");
            img.src = ev.target.result;
            img.hidden = false;
        };
        reader.readAsDataURL(file);
    });

    const cdCoverInput = document.getElementById("manual-cd-cover-input");
    if (cdCoverInput) {
        cdCoverInput.addEventListener("change", (e) => {
            const file = e.target.files[0];
            if (!file) return;
            manualCoverFile = file;
            document.getElementById("manual-cd-cover-filename").textContent = file.name;
            const reader = new FileReader();
            reader.onload = (ev) => {
                const img = document.getElementById("manual-cd-cover-preview");
                img.src = ev.target.result;
                img.hidden = false;
            };
            reader.readAsDataURL(file);
        });
    }

    const cdJanInput = document.getElementById("manual-cd-jan");
    if (cdJanInput) {
        cdJanInput.addEventListener("input", function () {
            this.value = this.value
                .replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xFEE0))
                .replace(/[\s\u3000\-－ー]/g, "");
        });
    }
}

function renderManualAuthorList() {
    const list = document.getElementById("manual-author-list");
    if (!list) return;
    list.innerHTML = "";
    manualAuthorIds.forEach((aid, idx) => {
        const author = manualAllAuthors.find((a) => a.id === aid);
        if (!author) return;
        const div = document.createElement("div");
        div.className = "edit-author-item";
        div.innerHTML = `
            <div class="edit-author-info">
                <div class="edit-author-name">${escapeHtml(author.name)}</div>
                <div class="edit-author-meta">
                    ${author.transcription ? `<span class="edit-author-yomi">${escapeHtml(author.transcription)}</span>` : ""}
                    ${author.ndl_id ? `<span class="edit-author-ndl">NDL: ${escapeHtml(author.ndl_id)}</span>` : ""}
                </div>
            </div>
            <button type="button" class="btn btn-xs btn-outline-danger" data-idx="${idx}">削除</button>
        `;
        div.querySelector("button").addEventListener("click", () => {
            manualAuthorIds.splice(idx, 1);
            renderManualAuthorList();
        });
        list.appendChild(div);
    });
}

function addManualAuthor() {
    if (!manualAuthorSelect) return;
    const aid = manualAuthorSelect.getValue();
    if (!aid) return;
    if (manualAuthorIds.includes(aid)) return;
    manualAuthorIds.push(aid);
    manualAuthorSelect.setValue(null);
    renderManualAuthorList();
}

async function submitManualBook(e) {
    e.preventDefault();
    const form = e.target;
    const fd = new FormData(form);
    const body = {};
    for (const [key, val] of fd.entries()) {
        if (key === "series_id" || key === "grand_series_id" || key === "series_number") {
            body[key] = val === "" ? null : parseInt(val, 10);
        } else if (key === "isbn" || key === "isdn") {
            body[key] = val === "" ? null : val;
        } else {
            body[key] = val === "" ? null : val;
        }
    }
    body.author_ids = manualAuthorIds.length > 0 ? manualAuthorIds : undefined;

    const statusEl = document.getElementById("manual-register-status");

    try {
        const res = await fetch("/api/books/manual", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
        });
        const data = await res.json();

        if (!res.ok) {
            statusEl.textContent = data.error || "登録に失敗しました";
            statusEl.className = "error";
            return;
        }

        const bookId = data.book.id;

        if (manualCoverFile && bookId) {
            const coverFd = new FormData();
            coverFd.append("cover", manualCoverFile);
            await fetch(`/api/books/${bookId}/cover`, { method: "POST", body: coverFd });
        }

        statusEl.textContent = `「${data.book.title}」を登録しました`;
        statusEl.className = "success";
        manualCoverFile = null;
        manualCoverPreview = null;
        manualAuthorIds = [];
        manualRendered = false;
        manualAuthorSelect = null;
        manualSeriesSelect = null;
        manualGrandSeriesSelect = null;
        form.reset();
        const preview = document.getElementById("manual-cover-preview");
        if (preview) preview.hidden = true;
        const fname = document.getElementById("manual-cover-filename");
        if (fname) fname.textContent = "";
        renderManualAuthorList();
    } catch (err) {
        statusEl.textContent = "通信エラーが発生しました";
        statusEl.className = "error";
    }
}

async function submitManualCd(e) {
    e.preventDefault();
    const title = document.getElementById("manual-cd-title").value.trim();
    if (!title) return;

    const body = {
        jan: document.querySelector("input[name=cd_jan]")?.value || null,
        title: title,
        artist: document.querySelector("input[name=cd_artist]")?.value || null,
        publisher: null,
        label: document.querySelector("input[name=cd_label]")?.value || null,
        catalog_number: document.querySelector("input[name=cd_catalog_number]")?.value || null,
        publish_date: document.querySelector("input[name=cd_publish_date]")?.value || null,
        description: document.querySelector("textarea[name=cd_description]")?.value || null,
        disc_count: parseInt(document.querySelector("input[name=cd_disc_count]")?.value) || null,
    };
    for (const key in body) {
        if (body[key] === "") body[key] = null;
    }

    const statusEl = document.getElementById("manual-cd-register-status");

    try {
        const res = await fetch("/api/cds", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
        });
        const data = await res.json();

        if (!res.ok) {
            statusEl.textContent = data.error || "登録に失敗しました";
            statusEl.className = "error";
            return;
        }

        const cdId = data.cd?.id || data.id;

        if (manualCoverFile && cdId) {
            const coverFd = new FormData();
            coverFd.append("cover", manualCoverFile);
            await fetch(`/api/cds/${cdId}/cover`, { method: "POST", body: coverFd });
        }

        statusEl.textContent = `「${data.cd?.title || data.title}」を登録しました`;
        statusEl.className = "success";
        manualCoverFile = null;
        manualCoverPreview = null;
        manualRendered = false;
        renderManualForm();
    } catch (err) {
        statusEl.textContent = "通信エラーが発生しました";
        statusEl.className = "error";
    }
}
