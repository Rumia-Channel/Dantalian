const registerForm = document.getElementById("register-form");
const isbnInput = document.getElementById("isbn-input");
const registerBtn = document.getElementById("register-btn");
const registerStatus = document.getElementById("register-status");
const bookGrid = document.getElementById("book-grid");
const bookCount = document.getElementById("book-count");

registerForm.addEventListener("submit", async (e) => {
    e.preventDefault();
    const isbn = isbnInput.value.trim().replace(/-/g, "");
    if (!isbn) return;

    registerBtn.disabled = true;
    registerStatus.textContent = "検索中...";
    registerStatus.className = "";

    try {
        const res = await fetch("/api/books", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ isbn }),
        });
        const data = await res.json();

        if (!res.ok) {
            registerStatus.textContent = data.error || "登録に失敗しました";
            registerStatus.className = "error";
            return;
        }

        const sourceLabel = data.source === "openbd" ? "OpenBD" : data.source === "ndl" ? "国立国会図書館" : "キャッシュ";
        registerStatus.textContent = `「${data.book.title}」を${sourceLabel}から登録しました`;
        registerStatus.className = "success";
        isbnInput.value = "";
        loadBooks();
    } catch (err) {
        registerStatus.textContent = "通信エラーが発生しました";
        registerStatus.className = "error";
    } finally {
        registerBtn.disabled = false;
    }
});

async function loadBooks() {
    try {
        const res = await fetch("/api/books");
        const books = await res.json();
        bookCount.textContent = `(${books.length}冊)`;
        renderBooks(books);
    } catch {
        bookGrid.innerHTML = '<p class="empty-state">読み込みに失敗しました</p>';
    }
}

function renderBooks(books) {
    if (books.length === 0) {
        bookGrid.innerHTML = '<p class="empty-state">ISBNで書籍を登録してください</p>';
        return;
    }

    bookGrid.innerHTML = books
        .map(
            (book) => `
        <div class="book-card" data-id="${book.id}">
            ${
                book.cover_url
                    ? `<img class="book-cover" src="${book.cover_url}" alt="${book.title}" loading="lazy" onerror="this.outerHTML='<div class=\\'book-cover-placeholder\\'>No Image</div>'">`
                    : '<div class="book-cover-placeholder">No Image</div>'
            }
            <div class="book-info">
                <div class="book-title">${escapeHtml(book.title)}</div>
                ${book.author ? `<div class="book-author">${escapeHtml(book.author)}</div>` : ""}
                ${book.publisher ? `<div class="book-meta">${escapeHtml(book.publisher)}</div>` : ""}
                <button class="book-delete" onclick="deleteBook(${book.id})">削除</button>
            </div>
        </div>`
        )
        .join("");
}

async function deleteBook(id) {
    if (!confirm("この書籍を削除しますか？")) return;
    try {
        const res = await fetch(`/api/books/${id}`, { method: "DELETE" });
        if (res.ok) loadBooks();
    } catch {}
}

function escapeHtml(text) {
    const div = document.createElement("div");
    div.textContent = text;
    return div.innerHTML;
}

loadBooks();
