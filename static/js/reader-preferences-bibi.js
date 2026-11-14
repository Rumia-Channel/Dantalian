Bibi.x({
    id: "DantalianReaderPreferences",
    description: "Default reading direction for PDF and image ZIP books",
    version: "1.0.0",
})(() => {
    E.add("bibi:is-going-to:process-package-spine", (spine) => {
        if (B.Type !== "PDF" && B.Type !== "ImageZIP") return;
        // Apply before spread construction; Bibi restores any per-book override afterward.
        spine.setAttribute("page-progression-direction", window.dantalianReaderPreferences.direction);
    });
});
