# Bibi リーダー組み込み仕様

Dantalian における電子書籍リーダー [Bibi](https://github.com/Rumia-Channel/bibi) のビルド・取り込み手順、ホスト連携仕様、および検証方法をまとめています。

---

## 1. 対象バージョンと役割分担

- **Bibi バージョン**: **2.3.7**（リポジトリ・commit・`bun.lock` / `LICENSE` の SHA-256・`sourceDateEpoch`・Node/Bun バージョンの正本は [`bibi.lock.json`](../bibi.lock.json)）
- **連携 API**: `schemaVersion: 1` の位置 API およびホスト限定シリーズ間ナビゲーション API（[Bibi 日本語仕様書](https://github.com/Rumia-Channel/bibi/blob/v2.3.7/HOST_SERIES_NAVIGATION_ja.md)）
- **対応フォーマット**: EPUB・PDF・画像 ZIP（PNG / JPEG / GIF / WebP / AVIF / BMP を自然順・サブフォルダ順で描画）

| コンポーネント | 責務 |
| --- | --- |
| **Bibi** | EPUB / PDF / 画像 ZIP の取得・描画、目次・検索・ページ送り、元ソース CFI に基づく位置・進捗算出、端末内 Automark、配布物整合性検証 |
| **Dantalian** | 認証・認可、書籍 ID と複数ファイル版の管理、独自プリセット配信、端末内 EPUB キャッシュ、シリーズ前後巻ナビゲーション API、narou.rs 直接閲覧導線 |

> **現在の接続範囲**: 読書位置・読み方向・表示設定はブラウザ内の `localStorage`（Bibi Automark / 読書設定）および IndexedDB（EPUB キャッシュ）で管理しています。サーバー側への読書位置保存・アカウント間同期は未接続です。

---

## 2. ビルドと配布物の取り込み

改行コード差（CRLF / LF）によるチェックサム不一致を防ぐため、クリーンな別チェックアウト（`core.autocrlf=false`）で固定ツールチェーン（[`bibi.lock.json`](../bibi.lock.json) 記載の Node / Bun）を使用してビルド・取り込みを行います。

```sh
# 1. 固定リファレンスをクリーンに取得・ビルド
git clone --config core.autocrlf=false https://github.com/Rumia-Channel/bibi.git .tmp/bibi-source
cd .tmp/bibi-source
git checkout --detach v2.3.7
bun install --frozen-lockfile
bun test
bun run build
bun run verify:dist
bunx playwright install --with-deps chromium webkit
bun run test:browser
bun run archive
cd ../..

# 2. 取り込みスクリプトの契約テストと配置・再検証
node --test scripts/bibi/distribution.test.mjs
node scripts/import_bibi.mjs .tmp/bibi-source
node scripts/import_bibi.mjs --verify
```

### 取り込みとキャッシュバスタの仕組み

- [`scripts/import_bibi.mjs`](../scripts/import_bibi.mjs) は上流 `__dist/bibi/` 全体を検証し、`static/vendor/bibi/`（Git 管理対象外）へ配置します。
- 上流の `index.html` と `build-manifest.json` は改変せず、Dantalian 側の以下を追加し `import-manifest.json` に全追加ファイルのダイジェストを記録します。
  - `LICENSE`
  - 独自プリセット [`scripts/bibi/dantalian.js`](../scripts/bibi/dantalian.js) → `presets/dantalian.js`
  - ホスト専用エントリページ `dantalian.html`（キャッシュ・読書設定・narou 拡張を読み込み、preset `dantalian` を固定）
- **CI パイプライン**: [`.github/workflows/build.yml`](../.github/workflows/build.yml) の `bibi` ジョブが成果物（`bibi-reader`）を 1 度だけビルドし、Native / Worker の各ジョブが同一成果物を検証して組み込みます。
- **Linux 環境での縦書きフォント**: 縦書き漢字の送りが崩れないよう、Linux ブラウザ検証時は `fonts-noto-cjk` 等を導入します（`scripts/install_ci_fonts.mjs`）。
- **WebKit の検証環境**: Bibi 2.3.7 の `test:browser` は iPad PWA 回帰を WebKit で実行します。Chromium だけでなく WebKit と OS 依存パッケージもインストールします。

### `bibi.lock.json` 更新時の注意

`lockSha256` と `licenseSha256` はチェックアウト後のファイルではなく Git の正規 LF blob から算出してください。

```sh
git -C .tmp/bibi-source cat-file blob HEAD:bun.lock | sha256sum
git -C .tmp/bibi-source cat-file blob HEAD:LICENSE | sha256sum
```

---

## 3. 配信・プリセット・セキュリティ設計

### スタンドアロン起動と同一 Origin 配信

- リーダーは `/vendor/bibi/dantalian.html` を**スタンドアロンページ**（`window === window.top`）として開きます。Jo / 外側 iframe による埋め込みは使用しません。
- HTML 自身が basename `dantalian` をプリセットとして固定しており、URL の `?preset=...` 指定だけでシリーズナビゲーションや戻る機能を許可することはありません。
- URL は `URLSearchParams` で構築し、API トークン等の認証情報をクエリ文字列に載せません。EPUB 取得は同一 Origin の認証 Cookie を引き継ぎます。

### 独自プリセットと Native / Worker 配信の要件

- **プリセット明示**: [`scripts/bibi/dantalian.js`](../scripts/bibi/dantalian.js) は `default.js` を自動継承しないため、`accept-pdf: 'yes'`、`keep-settings: 'yes'`、`use-full-height: true`、余白、メニューバー・ページ送り UI、展開対象 `['*']`（拡張子なし `/epubs/{file_hash}` 対応）を明示しています。
- **Native `/epubs/` Range 正規化**: [`src/epub.rs`](../src/epub.rs) のミドルウェアにより、ZIP 末尾取得の `Range: bytes=-65557` がファイルサイズより大きい場合は `bytes=0-` へ正規化して `206` を返します（小さな EPUB での `416` エラー回避）。
- **Worker → Wasabi リダイレクト**: `/epubs/{file_hash}` が署名付き Wasabi URL へ `302` リダイレクトする場合、Wasabi バケット側に `GET` / `HEAD` の CORS（`Access-Control-Expose-Headers: Content-Range, Accept-Ranges, Content-Length`、`Accept-Ranges: bytes`）が必要です。
- **CSP 方針**: `allow-scripts-in-content: false` を維持し、EPUB 本文内のスクリプト実行は無効化します。ホスト側の `_headers` CSP では、リダイレクト先 Wasabi Origin（`https:`）への `connect-src`、WASM 用の `wasm-unsafe-eval`、ZIP Worker / 本文 iframe 用の `worker-src` / `frame-src` (`'self' blob:`) を許可しています。

### iPad PWA の操作と EPUB の混在レイアウト

- **iPad の standalone PWA・ページ表示**では、ブラウザが認識した `click` と `pointerup` の座標でタップを処理します。開始座標のずれをページ送り・巻間移動のスワイプと誤認しないよう、この条件ではスワイプ操作を無効にします。左右タップ・中央メニュー・本文リンク・文字選択は維持します。通常の Safari タブ、PC、スクロール表示は対象外です。
- **リフロー本文と固定画像が混在する EPUB**では、左右の見開き指定を同じ spread に結合するのは両方が固定レイアウトの場合だけです。クレジット本文と挿絵の誤結合や、画像への二重の位置補正による画面外へのはみ出しを防ぎます。
- これらは Bibi 2.3.7 の変更であり、Dantalian のプリセット・位置 API・シリーズナビゲーション API の変更は不要です。

### PDF・画像 ZIP の既定読み方向

- 管理画面「設定 → 読書設定（このブラウザ）」からデフォルトの読み方向（右読み `rtl` / 左読み `ltr`、既定 `rtl`）を選択でき、[`static/js/reader-preferences.js`](../static/js/reader-preferences.js) が `localStorage`（`dantalian_reader_direction`）に保存します。
- [`static/js/reader-preferences-bibi.js`](../static/js/reader-preferences-bibi.js) が `bibi:is-going-to:process-package-spine` で PDF・画像 ZIP のデフォルト方向を設定します。作品ごとに UI で変更・保存した読み方向が優先され、EPUB には適用されません。

### 端末内 EPUB キャッシュ（IndexedDB）

- [`static/js/epub-cache.js`](../static/js/epub-cache.js) / [`epub-cache-reader.js`](../static/js/epub-cache-reader.js) / [`epub-cache-bibi.js`](../static/js/epub-cache-bibi.js) により、EPUB 本文をブラウザの IndexedDB へ LRU キャッシュします（既定上限 512 MiB、`0` で無効化＋全削除）。
- 通常書籍は登録済み `file_hash`、narou.rs は接続先・作品 ID・`last_update` を毎回オンラインで照合します。オフライン閲覧は行わず、認証失敗や上流 `503` 時に旧版を最新版として表示しません。
- PDF・画像 ZIP は対象外です。

### Web 小説（narou.rs）の直接取得

- narou.rs の作品は `/reader/?narou=<id>` から起動し、Dantalian の `/api/narou/novels/{id}/epub` が設定済み narou.rs の `/api/read/v1/novels/{id}/epub` へ `302` リダイレクトします。
- ホストプリセットはこの同一 Origin・非負整数 ID の宛先に限り `request-with-credentials: true` を有効化し、利用者の既存ログイン Cookie で直接取得します。Range 非対応サーバーが `200` で EPUB 全体を返した場合もそのまま端末内で展開し、二重取得しません。

---

## 4. `Bibi.API` と端末保存識別子

専用ページ内のスクリプトでは `Bibi.API`（`schemaVersion: 1`）を利用できます。

| メンバー | 概要 |
| --- | --- |
| `getState()` | `schemaVersion`, `sessionId`, `bookKey`, `contentHash`, `status` (`loading`/`ready`/`error`), `operable`, `error`, `initialRestore` を同期返却 |
| `getPosition()` | `location` (`{ p, cfi }`), `progress`, `progressBasis` (`spine-text` / `spine-items`), `atEnd`, `layout` を同期返却 |
| `restorePosition(location, expectedIdentity?)` | Promise。`bookKey` / `contentHash` を照合して復元し、不一致時は `IDENTITY_MISMATCH` |
| `subscribe(type, listener)` | `bibi:state-changed` / `bibi:position-changed` を購読し、解除関数を返却 |

- **端末保存 namespace**: `storage-namespace=dantalian` を指定し、`(namespace, storage-user, book-key, content-hash)` の組で位置・設定を分離します。
  - 通常書籍: `book-key=book:<id>`, `content-hash=<file_hash>`, `storage-user=local`
  - narou.rs: `storage-namespace=narou`, `book-key=narou:<id>@<接続先>`, `content-hash=<版識別子>`

---

## 5. Dantalian 側の導線とシリーズ間ナビゲーション

### 起動導線

- **トップ一覧 (`/`)**: 本カードクリックは詳細モーダルを表示し、詳細・編集画面の「読む」から `/reader/?book=<id>&file=<file_id>&from=...` を開きます。
- **本専用一覧 (`/books/`)**: 本カードおよびシリーズ展開内の本クリックで直接 `/reader/?book=<id>&from=...` を開きます（カードの「詳細」ボタンも併用可）。
- [`static/reader/index.html`](../static/reader/index.html)（[`static/js/reader.js`](../static/js/reader.js)）が API 認証・書籍・ファイルの存在確認を行い、`location.replace` で `/vendor/bibi/dantalian.html` へ遷移します（ブラウザの「戻る」で元の一覧へ直帰可能）。

### シリーズ間ナビゲーション (`GET /api/reader-navigation`)

Native / Worker 共通の読み取り専用エンドポイントです（`Cache-Control: no-store`）。

- **クエリパラメータ**:
  - `book_key`: 必須（`book:<id>`）
  - `content_hash`: 必須（該当書籍の `files` に登録済みの `file_hash`。削除済み版は `409 CONTENT_CHANGED`）
  - `return_to`: 起動元のパス（`/`・`/books/`・`/edit/` のみ許可、外部 URL 等は `/books/` へフォールバック）
- **巻順**: `series_number ASC NULLS LAST, id ASC`（大シリーズは巻順に代用しない。CD / オーディオブックは対象外）。未登録・非対応の隣接巻がある場合は飛ばさずに `status: "missing-file" | "unsupported-format"`, `href: null` を返します。
- **画面操作**: 本文末尾／表紙から外向き操作を行うと一旦「次の巻へ／前の巻へ」の案内を表示し、次の独立した操作で隣接巻へ遷移します。目次横の「戻る」で `returnTo` へ戻ります。

---

## 6. 今後の拡張予定（サーバー位置保存・同期）

将来的にサーバー側へ読書位置・読了状態を同期する場合に必要な追加作業です。

- 認証済みユーザー単位の識別（`storage-user` とサーバー認可の統合）
- 位置保存 DB スキーマおよび取得・更新 API（`user + book + contentHash + schemaVersion + location + progressBasis/progress + version`）
- 起動時の初期位置復元・エラー表示、変更イベントの debounce 保存キューと離脱時 flush
- 現在位置・最遠到達位置・明示的な読了状態（`atEnd` のみで読了扱いしない）の分離
- 複数端末／複数タブ間の競合検出とファイル版更新時の安全なハンドリング

---

## 7. 検証コマンド

```sh
# 配布物取り込みスクリプトの検証
node --test scripts/bibi/distribution.test.mjs
node scripts/import_bibi.mjs --verify

# 端末内 EPUB キャッシュとリーダーのブラウザ結合テスト（要クリーン Bibi ソース）
node scripts/epub-cache/browser.test.mjs --bibi-source .tmp/bibi-source

# iPad PWA のタップ・混在レイアウトの WebKit 回帰
(cd .tmp/bibi-source && node tests/browser/ipad-pwa-input.mjs)

# シリーズ間ナビゲーション API の HTTP 契約テスト（起動済みサーバーに対して実行）
READER_NAVIGATION_BACKEND=native WORKER_BASE_URL=http://127.0.0.1:3000 node --test worker/tests/reader_navigation_contract.mjs
```
