(function initHeader() {
    var app = document.getElementById("app");
    if (!app) return;
    var active = document.body.dataset.activeNav || "home";

    var header = document.createElement("div");
    header.id = "dantalian-header";
    header.innerHTML =
        '<h1>Dantalian</h1>' +
        '<nav>' +
            '<a href="/" id="nav-home">一覧</a>' +
            '<a href="/music/" id="nav-music">音楽</a>' +
            '<a href="/register/" id="nav-register">登録</a>' +
            '<a href="/manage/" id="nav-manage">管理</a>' +
            '<a href="/authors/" id="nav-authors">アーティスト</a>' +
            '<a href="/licenses/" id="nav-licenses">ライセンス</a>' +
        '</nav>' +
        '<div class="settings-bar">' +
            '<span class="settings-label">表示幅</span>' +
            '<div class="settings-buttons" id="width-buttons"></div>' +
            '<div class="settings-buttons auth-settings hidden" id="auth-settings">' +
                '<button type="button" class="width-btn auth-open" id="auth-open">APIログイン</button>' +
            '</div>' +
        '</div>';

    app.insertBefore(header, app.firstChild);

    var navEl = document.getElementById("nav-" + active);
    if (navEl) navEl.classList.add("active");

    var AUTH_STORAGE_KEY = "dantalian_api_token";
    var authDialog = null;
    var authPromise = null;
    var resolveAuth = null;
    var authButton = document.getElementById("auth-open");
    var authSettings = document.getElementById("auth-settings");
    var originalFetch = window.fetch.bind(window);
    var authRequirementPromise = null;

    function getAuthToken() {
        try {
            return (sessionStorage.getItem(AUTH_STORAGE_KEY) || "").trim();
        } catch {
            return "";
        }
    }

    function setAuthToken(token) {
        try {
            sessionStorage.setItem(AUTH_STORAGE_KEY, token);
        } catch {}
        try {
            document.cookie = "dantalian_api_token=" + encodeURIComponent(token) +
                "; Max-Age=28800; Path=/; SameSite=Strict; Secure";
        } catch {}
        updateAuthButton();
    }

    function clearAuthToken() {
        try {
            sessionStorage.removeItem(AUTH_STORAGE_KEY);
        } catch {}
        try {
            document.cookie = "dantalian_api_token=; Max-Age=0; Path=/; SameSite=Strict; Secure";
        } catch {}
        updateAuthButton();
    }

    function updateAuthButton() {
        if (!authButton) return;
        authButton.textContent = getAuthToken() ? "認証設定" : "APIログイン";
    }

    function setAuthRequired(required) {
        if (authSettings) authSettings.classList.toggle("hidden", !required);
        if (!required) clearAuthToken();
        updateAuthButton();
    }

    function readAuthRequirement() {
        if (!authRequirementPromise) {
            authRequirementPromise = originalFetch("/api/health", {
                cache: "no-store",
                credentials: "same-origin",
            })
                .then(async function (response) {
                    if (!response.ok) return response.status !== 404;
                    var body = await response.json().catch(function () { return null; });
                    return !body || body.authentication_required !== false;
                })
                .catch(function () {
                    return true;
                })
                .then(function (required) {
                    setAuthRequired(required);
                    return required;
                });
        }
        return authRequirementPromise;
    }

    function createAuthDialog() {
        if (authDialog) return;
        authDialog = document.createElement("div");
        authDialog.id = "auth-dialog-overlay";
        authDialog.className = "auth-dialog-overlay hidden";
        authDialog.innerHTML =
            '<form class="auth-dialog" role="dialog" aria-modal="true" aria-labelledby="auth-dialog-title">' +
                '<h2 id="auth-dialog-title">Dantalian APIログイン</h2>' +
                '<p id="auth-dialog-message">staging Worker APIを利用するにはAPIトークンが必要です。</p>' +
                '<label for="auth-dialog-token">APIトークン</label>' +
                '<input id="auth-dialog-token" type="password" autocomplete="current-password" required>' +
                '<div class="auth-dialog-actions">' +
                    '<button type="submit" class="btn btn-primary">ログイン</button>' +
                    '<button type="button" class="btn btn-secondary" id="auth-dialog-clear">認証情報を削除</button>' +
                '</div>' +
                '<small>トークンはこのタブのsessionStorageに保存され、音声・画像の取得用に同一origin cookieも設定されます。</small>' +
            '</form>';
        document.body.appendChild(authDialog);

        authDialog.querySelector("form").addEventListener("submit", function (event) {
            event.preventDefault();
            var input = document.getElementById("auth-dialog-token");
            var token = input.value.trim();
            if (!token) {
                input.setCustomValidity("APIトークンを入力してください");
                input.reportValidity();
                return;
            }
            input.setCustomValidity("");
            setAuthToken(token);
            authDialog.classList.add("hidden");
            if (resolveAuth) {
                var resolve = resolveAuth;
                resolveAuth = null;
                authPromise = null;
                resolve(token);
            }
        });

        authDialog.querySelector("#auth-dialog-clear").addEventListener("click", function () {
            clearAuthToken();
            authDialog.classList.add("hidden");
            window.location.reload();
        });
    }

    function showAuthDialog(message) {
        createAuthDialog();
        var messageEl = document.getElementById("auth-dialog-message");
        if (messageEl && message) messageEl.textContent = message;
        var input = document.getElementById("auth-dialog-token");
        if (input) {
            input.value = "";
            window.setTimeout(function () { input.focus(); }, 0);
        }
        authDialog.classList.remove("hidden");
    }

    function waitForToken() {
        var token = getAuthToken();
        if (token) return Promise.resolve(token);
        if (!authPromise) {
            authPromise = new Promise(function (resolve) {
                resolveAuth = resolve;
            });
            showAuthDialog();
        }
        return authPromise;
    }

    function protectedApiRequest(input) {
        var rawUrl = typeof input === "string"
            ? input
            : input instanceof URL
                ? input.href
                : input && input.url;
        if (!rawUrl) return false;
        try {
            var url = new URL(rawUrl, window.location.href);
            return url.origin === window.location.origin &&
                (url.pathname.startsWith("/api/") ||
                    url.pathname.startsWith("/audio/") ||
                    url.pathname.startsWith("/images/") ||
                    url.pathname.startsWith("/epubs/"));
        } catch {
            return false;
        }
    }

    window.fetch = async function (input, init) {
        if (!protectedApiRequest(input)) {
            return originalFetch(input, init);
        }

        if (!await readAuthRequirement()) {
            return originalFetch(input, init);
        }

        var token = await waitForToken();
        var headers = new Headers(input instanceof Request ? input.headers : undefined);
        if (init && init.headers) {
            new Headers(init.headers).forEach(function (value, key) {
                headers.set(key, value);
            });
        }
        if (token && !headers.has("authorization")) {
            headers.set("Authorization", "Bearer " + token);
        }

        var requestInit = Object.assign({}, init || {}, { headers: headers });
        var response = await originalFetch(input, requestInit);
        if (response.status === 401 && token === getAuthToken()) {
            clearAuthToken();
            authPromise = null;
            resolveAuth = null;
            showAuthDialog("APIトークンが無効です。正しいトークンを入力してください。");
        }
        return response;
    };

    if (authButton) {
        authButton.addEventListener("click", function () {
            showAuthDialog(getAuthToken()
                ? "現在の認証情報を変更できます。"
                : "staging Worker APIを利用するにはAPIトークンが必要です。");
        });
    }
    updateAuthButton();
})();
