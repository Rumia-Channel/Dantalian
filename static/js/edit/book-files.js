async function refreshBookFiles(bookId) {
    const response = await fetch(`/api/books/${bookId}`);
    if (!response.ok) throw new Error(`Book refresh failed: ${response.status}`);
    const book = await response.json();
    const summary = allBooks.find((item) => item.id === bookId);
    if (summary) Object.assign(summary, book);
    document.getElementById("edit-epub-info").innerHTML = renderBookFileList(book, true);
}

document.getElementById("edit-content").addEventListener("change", async (event) => {
    const input = event.target;
    if (input.id !== "edit-epub-input" || !input.files.length) return;
    const bookId = parseInt(new URLSearchParams(location.search).get("book"), 10);
    if (!bookId) return;
    input.disabled = true;
    try {
        for (const attachment of Array.from(input.files)) {
            const response = await uploadFileWithChunks(`/api/books/${bookId}/files`, "file", attachment, "epub");
            if (!response.ok) {
                alert(`${attachment.name}: ${await describeUploadError(response, "ファイル")}`);
                break;
            }
        }
        await refreshBookFiles(bookId);
    } catch (error) {
        console.error("File upload error:", error);
        alert("ファイルのアップロード中に通信エラーが発生しました");
    } finally {
        input.value = "";
        input.disabled = false;
    }
});

async function deleteBookFile(bookId, fileId) {
    const ok = await showConfirm({ message: "このファイルを削除しますか？", okLabel: "削除" });
    if (!ok) return;
    try {
        const response = await fetch(`/api/books/${bookId}/files/${fileId}`, { method: "DELETE" });
        if (response.ok) {
            await refreshBookFiles(bookId);
        } else {
            alert("ファイルを削除できませんでした");
        }
    } catch {
        alert("ファイルの削除中に通信エラーが発生しました");
    }
}
