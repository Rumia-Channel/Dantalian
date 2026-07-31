// --- Tab switching ---
const tabs = document.querySelectorAll(".register-tab");
const isbnPanel = document.getElementById("isbn-panel");
const isdnPanel = document.getElementById("isdn-panel");
const manualPanel = document.getElementById("manual-panel");

const cdPanel = document.getElementById("cd-panel");
const audiobookPanel = document.getElementById("audiobook-panel");

tabs.forEach((tab) => {
    tab.addEventListener("click", () => {
        tabs.forEach((t) => t.classList.remove("active"));
        tab.classList.add("active");
        const target = tab.dataset.tab;
        tabs.forEach((t) => t.setAttribute("aria-selected", String(t === tab)));
        isbnPanel.hidden = target !== "isbn";
        isdnPanel.hidden = target !== "isdn";
        cdPanel.hidden = target !== "cd";
        audiobookPanel.hidden = target !== "audiobook";
        manualPanel.hidden = target !== "manual";
        if (target === "manual") {
            renderManualForm();
        }
    });
});
