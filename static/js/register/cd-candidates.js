let cdCandidateModal = null;

function ensureCdCandidateModal() {
    if (cdCandidateModal) return cdCandidateModal;

    cdCandidateModal = document.createElement("div");
    cdCandidateModal.className = "cd-candidate-modal";
    cdCandidateModal.hidden = true;
    cdCandidateModal.innerHTML = `
        <section class="cd-candidate-dialog" role="dialog" aria-modal="true" aria-labelledby="cd-candidate-title">
            <header class="cd-candidate-header">
                <div>
                    <span class="cd-candidate-kicker">MUSICBRAINZ MATCH</span>
                    <h2 id="cd-candidate-title">登録するCDを選択</h2>
                </div>
                <button type="button" class="cd-candidate-close" data-cd-candidate-close aria-label="閉じる">×</button>
            </header>
            <div class="cd-candidate-body">
                <p class="cd-candidate-intro" data-cd-candidate-intro></p>
                <div class="cd-candidate-list" data-cd-candidate-list></div>
                <p class="cd-candidate-status" data-cd-candidate-status role="status"></p>
            </div>
        </section>`;
    document.body.appendChild(cdCandidateModal);

    cdCandidateModal.addEventListener("click", async (event) => {
        if (event.target === cdCandidateModal || event.target.closest("[data-cd-candidate-close]")) {
            cdCandidateModal.hidden = true;
            return;
        }

        const button = event.target.closest("[data-mb-release-id]");
        if (!button || button.disabled) return;
        const context = cdCandidateModal._context;
        if (!context) return;

        cdCandidateModal.querySelectorAll("[data-mb-release-id]").forEach((item) => {
            item.disabled = true;
        });
        setCdCandidateStatus("選択したreleaseを取得して登録しています...");

        try {
            const body = {
                jan: context.jan,
                musicbrainz_release_id: button.dataset.mbReleaseId,
            };
            if (context.parentBookId) body.parent_book_id = context.parentBookId;
            if (context.mediaType) body.media_type = context.mediaType;

            const res = await fetch("/api/cds", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify(body),
            });
            const data = await res.json();
            if (!res.ok) throw new Error(data.error || "CDの登録に失敗しました");

            cdCandidateModal.hidden = true;
            if (typeof context.onRegistered === "function") context.onRegistered(data);
        } catch (err) {
            cdCandidateModal.querySelectorAll("[data-mb-release-id]").forEach((item) => {
                item.disabled = false;
            });
            setCdCandidateStatus(err.message || "CDの登録に失敗しました", true);
        }
    });

    document.addEventListener("keydown", (event) => {
        if (event.key === "Escape" && cdCandidateModal && !cdCandidateModal.hidden) {
            cdCandidateModal.hidden = true;
        }
    });

    return cdCandidateModal;
}

function setCdCandidateStatus(message, isError = false) {
    if (!cdCandidateModal) return;
    const status = cdCandidateModal.querySelector("[data-cd-candidate-status]");
    status.textContent = message || "";
    status.classList.toggle("error", isError);
}

function candidateMeta(candidate) {
    const first = [candidate.artist, candidate.date, candidate.country]
        .filter((value) => value && String(value).trim())
        .map((value) => escapeHtml(String(value).trim()))
        .join(" · ");
    const second = [candidate.label, candidate.catalog_number]
        .filter((value) => value && String(value).trim())
        .map((value) => escapeHtml(String(value).trim()))
        .join(" / ");
    const format = [
        second,
        candidate.disc_count ? `${candidate.disc_count}枚組` : "",
        candidate.track_count ? `${candidate.track_count}曲` : "",
        candidate.barcode ? `EAN ${escapeHtml(candidate.barcode)}` : "",
    ].filter(Boolean).join(" · ");
    return [first, format].filter(Boolean).join("<br>");
}

function openMusicBrainzCandidatePicker({ jan, parentBookId = null, amazonTitle = "", candidates, mediaType = null, onRegistered } = {}) {
    const list = Array.isArray(candidates)
        ? candidates
        : (Array.isArray(candidates?.candidates) ? candidates.candidates : []);
    if (!jan) return;

    const modal = ensureCdCandidateModal();
    modal._context = { jan, parentBookId, mediaType, onRegistered };
    modal.querySelector("[data-cd-candidate-intro]").innerHTML =
        `Amazonで取得したタイトル「${escapeHtml(amazonTitle || "（タイトル不明）")}」に一致する候補があります。`;
    modal.querySelector("[data-cd-candidate-list]").innerHTML = list.length > 0
        ? list.map((candidate) => `
        <button type="button" class="cd-candidate-item" data-mb-release-id="${escapeAttr(candidate.id)}">
            <strong>${escapeHtml(candidate.title || "タイトル不明")}</strong>
            <span>${candidateMeta(candidate)}</span>
        </button>`).join("")
        : `<p class="cd-candidate-status error">候補データを表示できませんでした。もう一度検索してください。</p>`;
    setCdCandidateStatus(list.length > 0 ? "" : "候補データが空です", list.length === 0);
    modal.hidden = false;
    modal.querySelector("[data-mb-release-id]")?.focus();
}
