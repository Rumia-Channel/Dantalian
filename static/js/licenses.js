(function initLicenseFilter() {
    var input = document.getElementById("license-filter");
    var status = document.getElementById("license-filter-status");
    var entries = Array.prototype.slice.call(document.querySelectorAll(".license-entry"));

    if (!input || !status || entries.length === 0) return;

    function update() {
        var query = input.value.trim().toLocaleLowerCase();
        var visible = 0;

        entries.forEach(function (entry) {
            var matches = !query || entry.dataset.licenseSearch.toLocaleLowerCase().includes(query);
            entry.classList.toggle("is-filtered", !matches);
            if (matches) visible += 1;
        });

        status.textContent = query
            ? visible + " / " + entries.length + " 件を表示"
            : entries.length + " 件のライセンスを表示";
    }

    input.addEventListener("input", update);
    update();
})();
