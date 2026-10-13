# Bibi 組み込み仕様と Dantalian 側の追加・接続作業

## 対象バージョンと役割

Bibi **2.1.0** のホスト連携機能を使用する。取り込み対象は [Rumia-Channel/bibi の commit `27b05f902718c835e063e96f28515ec1d2d621f2`](https://github.com/Rumia-Channel/bibi/commit/27b05f902718c835e063e96f28515ec1d2d621f2)（`v2.1.0` の master 統合）。移動するブランチ先端ではなく、[`bibi.lock.json`](../bibi.lock.json) に repository・SHA・lock/license の SHA-256・sourceDateEpoch・Node/Bun を固定する。既存の `v2.0.0` タグには本書の新 API は含まれない。

- **Bibi**: EPUB の取得・展開・描画、目次・移動、操作可能状態、元本文に対応した位置、進捗、端末内 Automark、公開 API と配布物検証。
- **Dantalian**: 認証・認可、書籍 ID とファイル版の管理、サーバー保存・同期、競合解決、現在位置と最遠位置、明示的な読了状態、ライブラリ一覧表示。
- Dantalian 側では固定成果物の検証・取り込み、独自 preset、Native/Worker の静的配信と CI の共通成果物化に加え、**本のクリックから Bibi の standalone 専用ページを起動し、既存書籍 API の登録済み EPUB を開く導線を接続済み**。Jo / iframe にリーダーを埋め込まない。進捗・ブラウザ内の再開位置は Bibi 自身が管理する。**サーバーへの位置保存・同期、アカウント別の位置管理は未接続**。Bibi は Dantalian の保存 API を直接呼ばない。

## Bibi 側で実装された機能（優先順）

| 優先度 | 実装済みの内容 | ホストからの利用方法 |
| --- | --- | --- |
| P0 | Jo の受信解析、型を維持する JSON 送信、移動・focus コマンド名の修正 | 配布版 `and/jo.js` の `Bucket` を使う。ソースの一時バンドルやローカル修正は不要。 |
| P0 | schemaVersion 1 の状態・位置取得、確定位置通知、初期・実行中の復元と失敗結果 | `Bucket.api.ready/getState/getPosition/restorePosition/subscribe`。 |
| P0 | requestId の対応、iframe window と exact origin の検証、session/book/file の分離 | API を介して要求する。古い書籍のイベントを新しい書籍に保存しない。 |
| P0 | Jo を含む必要ファイルのビルド・ZIP 両方での欠落検出 | `bun run build`、`bun run verify:dist`、`bun run archive`。 |
| P1 | 長い段落の UTF-16 文字位置を含む元ソース point CFI、挿絵移動後も元の spine を保持 | `position.location` を保存。ページ番号・IIPP・進捗だけを復元キーにしない。 |
| P1 | ページ数と独立した進捗、終端表示状態 | `progress` と `progressBasis`。`atEnd` は読了フラグに直結させない。 |
| P1 | ホスト namespace・書籍・ファイル版・ユーザー別の端末保存領域 | `storage-namespace/storage-user/book-key/content-hash`。 |
| P1 | version/SHA/lock/toolchain/file digests を含む manifest、再現可能な ZIP | `build-manifest.json` を検証して取り込む。 |
| P1 | 配布物のブラウザ回帰と既存 CI への組み込み | `bun run test:browser`。任意配置パス、複数 iframe、リフロー・固定レイアウト等を検証。 |

追加の修正として、離れた位置へ復元する際に移動先 spread が非表示のままとなり、元の位置が返る経路を修正した。ZIP Worker と全体取得の資格情報ポリシーも揃え、同一 origin の cookie を送信するようにした。

## 1. 配布物の取り込み

クリーンな別 checkout で [`bibi.lock.json`](../bibi.lock.json) の固定 ref・toolchain と `bun.lock` を使用する。ローカルの作業中ディレクトリや古い `__dist` をそのままコピーしない。次は Dantalian のルートで実行する:

```sh
git clone --config core.autocrlf=false https://github.com/Rumia-Channel/bibi.git .tmp/bibi-source
cd .tmp/bibi-source
git checkout --detach 27b05f902718c835e063e96f28515ec1d2d621f2
bun install --frozen-lockfile
bun test
bun run build
bun run verify:dist
bunx playwright install chromium
bun run test:browser
bun run archive
cd ../..
node --test scripts/bibi/distribution.test.mjs
node scripts/import_bibi.mjs .tmp/bibi-source
node scripts/import_bibi.mjs --verify
```

確認環境は Bun 1.4.2 / Node.js 24.21.0。再現性を比較する場合はこれらも同じバージョンに固定する。CI/Linux では Chromium のインストールに `bunx playwright install --with-deps chromium` を使い、`fonts-noto-cjk` もインストールする。検証 EPUB は `Noto Serif CJK JP` / Windows `MS Mincho` を明示する。英字 serif のフォールバックだけでは縦書き漢字の送りが 0 になり重なって描画される環境があるため、実運用でも適切なシステムフォントまたは EPUB 埋め込みフォントを用意する。

固定 checksum は Git の正規バイト列（LF）の `bun.lock` / `LICENSE` から取得する。Windows の CRLF checkout から採った checksum は Linux CI と一致しないため、上記 clone では `core.autocrlf=false` を保存する。ホスト独自 preset も `.gitattributes` の `eol=lf` で固定する。改行差を理由に manifest 検証を省略・緩和しない。

[`scripts/import_bibi.mjs`](../scripts/import_bibi.mjs) が検証済みの `__dist/bibi/` 全体を `static/vendor/bibi/` に配置する。SHA・クリーンな checkout・lock・toolchain・必須ファイル・一覧・サイズ・digest を確認し、ステージングの検証に成功してから既存配布物と入れ替える。検証失敗時は既存配布物を変更しない。`--verify` は配置済み成果物を再検証し、`--destination DIRECTORY` で Worker の複製先等も確認できる。生成ディレクトリは Git 管理しない。配下の相対関係を維持し `/vendor/bibi/` で配信する。デモ・サンプル本は不要。Jo だけ、reader JS だけを抜き出す配布は不可。

`bibi/build-manifest.json` の主要フィールド:

| フィールド | 取り込み時の検証 |
| --- | --- |
| `schemaVersion` / `version` | `1` / `2.1.0` であること。 |
| `source.commit` / `source.dirty` | 上記固定 SHA / `false`。作業中ビルドは `dirty:true` を正直に記録するため、正式取り込みには使わない。 |
| `source.lockSha256` | 固定 checkout の `bun.lock` SHA-256 と一致。 |
| `source.sourceDateEpoch` | `SOURCE_DATE_EPOCH` または commit 時刻由来の固定値。 |
| `toolchain` | 固定した Node/Bun のバージョン。 |
| `files` | reader 配下の各ファイルの相対パス、サイズ、SHA-256。manifest 自身は循環を避けて一覧に含まれない。 |

ビルドと ZIP は reader、Jo、CSS、既定 preset、sanitizer、ZIP bootstrap/vendor Worker を必須検証する。ZIP は CRC、パス、安全でないエントリー、一覧とダイジェストも検証する。既存 ZIP は `bun zzz/distribution.mjs verify-zip <zip-path>` で確認できる。manifest は完全性の検出であり署名ではないので、取得元と固定 ref の信頼も必要。

ZIP はエントリー順・UTC/DOS 時刻・権限を固定する。同じソース・lock・ツールチェーン・環境入力なら同じバイト列となる。異なるツールバージョン、dirty 状態、改変済み成果物まで同一という意味ではない。ホスト独自 preset は別ファイルとして管理し、元の manifest と独自追加分の一覧・ハッシュを取り込み側でも記録する。

取り込み時に upstream の `build-manifest.json` は改変せず、`LICENSE` と [`scripts/bibi/dantalian.js`](../scripts/bibi/dantalian.js) を追加する。`import-manifest.json` が元 manifest の digest とホスト追加分の一覧・サイズ・SHA-256 を記録する。配置済み検証では追加分の自己申告だけでなく、固定 LICENSE と現在の tracked preset のバイト列も照合する。

[`build.yml`](../.github/workflows/build.yml) の `bibi` job が frozen install、unit/browser 検証、ビルド、取り込みを行い、`bibi-reader` artifact を一度だけ作る。Native・Worker・develop・staging・production は同じ artifact を取得する。Native は Cargo 実行前、Worker/デプロイは `worker/build_assets.mjs` の冒頭で再検証する。Worker は旧 reader ディレクトリを削除してから静的配布物を複製し、削除された upstream ファイルを残さない。Native の `build.rs` と Worker のアセットハッシュには manifest・preset・LICENSE も反映する。`v0.1.16` の [Linux CI](https://github.com/Rumia-Channel/Dantalian/actions/runs/38429352764) で Bibi・Native・Worker・本番デプロイが成功した。

## 2. EPUB 配信と preset

### 同一 origin を既定にする

ホストと `/vendor/bibi/` は同じ origin で配信する。`book` には `/epubs/{file_hash}` を指定できる。URL は `URLSearchParams` で構築し、API の bearer token やユーザーの認証情報を query に載せない。

拡張子のない EPUB/ZIP endpoint は、ホスト側で `/vendor/bibi/presets/dantalian.js` を追加して次の設定を配信し、reader URL に `preset=dantalian` を指定する:

```javascript
Bibi.preset({
    'extract-if-necessary': ['*'],
    'autostart': 'yes',
    'resume-from-last-position': 'yes',
    'keep-settings': 'yes',
    'use-menubar': 'yes',
    'use-arrows': 'yes',
    'use-keys': 'yes',
    'use-slider': 'yes',
    'use-nombre': 'yes',
    'allow-scripts-in-content': false,
    'request-with-credentials': false
});
```

この preset は EPUB を返す Dantalian の書籍 endpoint 用。2.1.0 の判定では空拡張子 `''` を含めるとその分岐が先に返り、`['.epub', '.zip', '']` でも `.epub` 入力をフォルダーとして取得してしまう。実配信で確認したため、拡張子なし・`.epub` の両方を展開対象にする文書化済みの `['*']` を使用する。

独自 preset は `default.js` の操作 UI 設定を自動継承しない。目次・表示設定のメニューバー、左右のページ送り、キー操作、位置スライダー、ページ表示を明示的に有効にする。自動起動だけを指定すると本文を取得できてもこれらの操作が無効になる。

Native の `/epubs/` は `src/epub.rs` の middleware を経由する。Bibi が ZIP の末尾を読む `Range: bytes=-65557` に対し、非空ファイルが指定 suffix より短ければ `bytes=0-` に正規化する。[RFC 9110 §14.1.2](https://www.rfc-editor.org/rfc/rfc9110.html#section-14.1.2) に合わせて全体を `206` で返し、小さな EPUB が既存 `ServeDir` の `416` で開けなくなる問題を解消する。その他の Range・ディレクトリ配信・ストリーミング本体は `ServeDir` に委ねる。

`request-with-credentials:false` は**すべての cookie を禁止する意味ではない**。ZIP Worker/全体取得は `credentials:'same-origin'` を使用し、同一 origin の認証 cookie は送る。クロス origin には送らない。`true` の場合だけ `include` を使用する。Native endpoint の認証・認可はサーバー側で行い、未認証に EPUB 内容を返さない。

### Worker → Wasabi のリダイレクト

Worker の `/epubs/{file_hash}` が署名付き Wasabi URL にリダイレクトする構成では、最終ストレージ応答の CORS と Range を確認する。Worker 自体が EPUB 全体を直接配信する前提ではない。

- 同一 origin の開始 endpoint は cookie を受ける。`same-origin` のままの別 origin リダイレクト先にはホストの cookie を送らない。
- 必要な `Access-Control-Allow-Origin`、必要なら Range の OPTIONS、`Accept-Ranges: bytes`、正しい `206` / `Content-Range` / `Content-Length` を返す。
- JS が Range 情報を読めるよう `Access-Control-Expose-Headers: Content-Range, Accept-Ranges, Content-Length` を設定する。
- 資格情報なしの署名付きストレージ応答では `Allow-Origin:*` を使用できる。credentialed CORS では exact origin と `Allow-Credentials:true` が必要で、`*` を使わない。
- 署名の期限が複数 Range 要求を完了するまで有効か、期限切れ時にホストがどう再取得・位置復元するかを決める。署名 URL は短命の capability であり、長期認証 token を reader URL に渡す代替ではない。
- ローカルの cookie 認証・別 origin 署名相当リダイレクト・CORS/Range は確認済み。`v0.1.16` の認証済み Firefox でも本番 Worker → Wasabi の登録済み EPUB が開き、最終 Range 応答 `206` と実際の表紙表示・ページ移動を確認した。署名失効後の再取得まで検証したものではない。

### origin / CSP

- Bibi は同一 origin の standalone 専用ページとして開く。Jo、親ページとの message bridge、リーダーを包む iframe は使用しない。Bibi 内部の EPUB 本文用 iframe とは区別する。
- 起動ページの書籍 API 認証は共通の `static/js/auth.js` を使用する。EPUB 取得は同一 origin の認証 cookie を引き継ぎ、token を reader URL に載せない。
- `allow-scripts-in-content:false` を維持する。reader 自身の JavaScript と EPUB 内のスクリプトは別であり、前者まで無効化すると描画できない。
- CSP は reader script、ZIP Worker と WASM、`blob:` の本文 iframe、本文 styles、画像・フォント、book の取得先を許可する必要がある。少なくとも `worker-src` / `frame-src` / `connect-src` / `img-src` / `font-src` を検討し、WASM はブラウザの `wasm-unsafe-eval` 要件も確認する。任意の `script-src *` や EPUB script 許可で回避しない。

## 3. 専用リーダーページ内の公開 API

Bibi の専用ページ本体で `Bibi.API` を使用する。Dantalian の起動ページは書籍情報を確認して専用ページへ遷移するだけであり、遷移前の window から API を操作しない。以下は専用ページで使う公開 API の例で、未作成のサーバー保存 API を呼ぶ実装ではない。

```javascript
const API = Bibi.API;
const unsubscribePosition = API.subscribe('bibi:position-changed', Event => {
    console.log(Event.schemaVersion, Event.sessionId,
        Event.bookKey, Event.contentHash, Event.position);
});
const unsubscribeState = API.subscribe('bibi:state-changed', Event => {
    console.log(Event.state.status, Event.state.operable, Event.state.initialRestore);
});
```

操作可能状態での位置取得と、保存レコードの識別子を照合する実行中の復元:

```javascript
const State = Bibi.API.getState();
const Position = Bibi.API.getPosition();

function restoreSavedPosition(SavedRecord) {
    return Bibi.API.restorePosition(SavedRecord.location, {
        bookKey: SavedRecord.bookKey,
        contentHash: SavedRecord.contentHash
    });
}
```

保存レコードを復元するときは、**現在の State から作り直した識別子ではなく保存レコードの bookKey/contentHash** を渡す。異なる本・ファイル版なら `IDENTITY_MISMATCH` となり移動しない。購読を終了するときは返された `unsubscribePosition()` / `unsubscribeState()` を呼ぶ。本の切り替えは新しい専用ページへの遷移で行い、内部の `R` / `E` / Biscuit に依存しない。

### schemaVersion 1 の契約

| API / フィールド | 意味 |
| --- | --- |
| `getState()` | 同期で schemaVersion、sessionId、bookKey、contentHash、status、operable、error、initialRestore を返す。status は loading/ready/error。 |
| `getPosition()` | 同期で location、progress、progressBasis、atEnd、layout を返す。 |
| `restorePosition(location, expectedIdentity?)` | Promise。lazy spine を必要に応じロードして復元し、requestedLocation と実際の position を返す。 |
| `subscribe(type, listener)` | state-changed / position-changed の envelope。購読解除関数を返す。 |
| `position.layout` | iipp/pageIndex/pageCount。診断値でありサーバーの復元キーではない。 |

通知の位置は `Event.position.location` にある。schemaVersion/sessionId/bookKey/contentHash は envelope に付く。読み込み後も resize・表示モード変更・復元の途中は `state.operable:false` になり得る。state-changed で操作可能状態を確認し、失敗は `Error.code` で扱う: NOT_READY、BUSY、INVALID_LOCATION、LOCATION_NOT_FOUND、INVALID_IDENTITY、IDENTITY_MISMATCH、RESTORE_FAILED、LOAD_FAILED。失敗を「保存位置がないので成功」と扱わず UI に状態を残す。

## 4. 保存する位置・進捗・終端

### 元ソースの位置

`position.location` は `{ p, cfi }`。推奨復元キーは**文字 offset を含む cfi**で、必要なら p も一緒に保存する。元の sanitization 後の DOM を pagination/挿絵移動より前に記録するため、表示用 DOM の nth-child をホストが取り直す必要はない。

- 対応形式: linear spine の `/6/<偶数>!/4/...` point CFI と UTF-16 文字 offset。長い段落の途中に復元できる。P は original linear item と element の位置で、段落内の文字位置までは表さない。
- 両方あれば CFI 優先。無効な CFI を P にすり替えたり範囲外を末尾に clamp したりしない。
- range CFI、nested indirect CFI、nonlinear spine、存在しないノード・文字 offset は対象外/失敗。CFI 全仕様への対応を意味しない。
- lazy item は live restore 時にロードしてから解決する。挿絵が隣接本文へ移されても元の spine の CFI を維持する。右の挿絵と左の後続本文が同じ表示ページに並ぶ場合も、DOM の読み順で画像・本文を一緒に走査し、先頭の挿絵ではなく後続本文の位置を誤って返さない。
- 実際の返却位置は表示ページの先頭になることがあり、要求文字位置と数値が完全一致するとは限らない。復元結果と操作可能状態を確認する。

初期優先順は **明示 location/従来 URL destination → 端末 Automark → 本の先頭**。`location` は JSON を URLSearchParams に渡す。従来の `p/epubcfi/nav/iipp/edge` を新しい保存形式に置き換えなくても開けるが、ホスト保存は新 API の canonical location を使う。明示位置が無効なら `state.initialRestore.ok:false` と error を通知して先頭を表示する。`initialRestore.position` には実際の位置が入る。明示位置を使わない通常起動では initialRestore は null。

### 進捗の定義

linear spine item 数を N、0-based item index を i とする。

- `spine-text`: `(i + 元本文 UTF-16 文字位置 / その item の本文 UTF-16 文字数) / N`。spine 項目は等重み。whitespace-only ノードは本文分母から除く。本文長全体で全章を重み付けした値ではない。
- `spine-items`: 固定レイアウト/本文テキストのない項目は `(i + 1) / N`。
- フォント・画面幅で変わる pageCount は使わない。progressBasis も保存すれば意味を判別できる。
- `atEnd:true` は最後のページが現在表示されている状態。**読了・完読時間・ユーザーの意図ではない**。目次から末尾へ飛んでも読了を強制しない。

## 5. 端末保存 namespace とユーザー分離

`storage-namespace=dantalian` を指定すると Book 用 localStorage ラベルは `(namespace, storage-user, book-key, content-hash)` の tuple になる。book-key/content-hash は必須。不足は INVALID_STORAGE_IDENTITY。値はホストの不透明な識別子にし、token や秘密情報を保存ラベルに使わない。

- 同じ EPUB の書誌 identifier でも書籍 ID・ファイル hash・ユーザーが違えば混ざらない。
- 同じ origin 内なら `/vendor/bibi/` の配置場所・preset パスが変わっても同じ tuple で再開できる。
- storage-user 未指定は空文字の共通区分になるので、ログインユーザー/ゲストを明示的に分ける。アカウント切替時は reader を破棄し、ホストの保存キューも切り替える。
- origin が変わると localStorage 自体が別領域。namespace は端末間同期やサーバー認可を提供しない。UI/global settings の保存領域とは区別する。
- namespace がない旧経路は維持される。`sugar-for-biscuits` は preset で `allow-sugar-for-biscuits` が有効な場合、既存の Bibi/preset/EPUB-ID ラベルに**追加**される識別子であり、配置パスを置き換えるものではない。2.1.0 の新規組み込みでは explicit namespace を推奨する。

## 6. Dantalian 側で必要な追加・接続作業（優先順）

固定成果物の取り込み・静的配信、Native の suffix Range 修正、本のクリックからの読書画面起動は実装済み。残るサーバー保存・同期等は既存機能・作業中変更との重複を確認しながら接続する。

### 接続済みの読書画面

- [`static/js/book-links.js`](../static/js/book-links.js) が本の読書 URL を組み立てる。一覧・本一覧の本カードとシリーズ内の本から `/reader/?book=<id>&from=<戻り先>` を開く。カードの「詳細」は従来の `showDetail` を維持し、CD・オーディオブックは従来どおり詳細を開く。
- [`static/reader/index.html`](../static/reader/index.html)・[`static/css/reader.css`](../static/css/reader.css)・[`static/js/reader.js`](../static/js/reader.js) は起動・エラー案内ページを構成する。Native は `/reader`・`/reader/` を配信し、Worker は既存 ASSETS 配信を使用する。
- 既存の `/api/books/{id}` で書籍・`epub_file_hash` を取得し、同一 origin の `/epubs/{hash}` を指定した `/vendor/bibi/` へ `location.replace` で遷移する。Jo / Bucket と外側の iframe・一覧ヘッダー・独自ツールバーは廃止した。目次・表示設定・位置表示は Bibi 自身の UI に任せる。
- API 認証は `header.js` から分離した共通の `auth.js` を全ページで使用する。書籍不存在・EPUB 未登録・PDF は起動ページに表示し、書籍情報や元ファイルへの導線を残す。EPUB 読み込み後のエラーは Bibi が表示する。エラー案内の戻り先は同一 origin の一覧・本一覧・編集に限定する。一覧から開いた正常な読書画面では、ブラウザの「戻る」で起動ページを挟まず一覧に戻る。
- Automark は `storage-namespace=dantalian`、`storage-user=local`、`book-key=book:<id>`、`content-hash=<epub_file_hash>` で分離する。現時点の `local` はブラウザ内の共通区分であり、ログインユーザーの識別子ではない。読書位置 DB や `reading_status` は更新しない。

### 残る追加・接続作業

| 優先度 | 追加・接続対象 | 完了条件 |
| --- | --- | --- |
| P0 | 準備済み: 固定 SHA/lock/toolchain の成果物取り込み、独自 preset と静的配信 | manifest と Jo/Worker を検証した配布物だけで `/vendor/bibi/` の画面が動く。 |
| P0 | 認証済み user/book/file の reader 起動と切替 | 身元はサーバー側で確定し、他ユーザーのファイル・位置を取得できない。旧 session の保存要求を次の書籍へ流用しない。 |
| P0 | 位置保存 DB と認可済み取得/更新 API | user + book + contentHash + schemaVersion + location + progressBasis/progress + 更新 version/時刻を管理。ファイル更新前の CFI を新ファイルに無条件適用しない。 |
| P0 | 初期サーバー位置取得と live restore、失敗表示 | 保存済み identity を確認して明示初期位置を渡す。復元不能・ロード失敗を表示し、無言で古い/別の本の位置を保存しない。 |
| P1 | 確定位置イベントの保存キュー | 操作中/BUSY を除き debounce・最後の位置を扱う。離脱時の flush と再開・オフライン時の扱い、旧 session 要求の失効を定義する。 |
| P1 | 現在位置・最遠到達位置・明示読了を分離 | 後戻りしても現在位置を保存。furthest は別の値、finished は明示操作等のポリシーで更新。atEnd だけでは finished にしない。 |
| P1 | 端末間・複数タブの競合とファイル版更新 | サーバー version/ETag 等で競合を検出し、古い端末の遅延書き込みを判別。別 contentHash は別位置として扱う。 |
| P2 | ライブラリ一覧と再開 UX | 未読/読書中/読了、現在・最遠進捗、最後に読んだ時刻、復元失敗/ファイル更新の状態を表示。 |

保存 API の endpoint 名や DB カラムは Dantalian の既存設計に合わせて決める。本書の Bibi API と異なり、未作成 endpoint を呼べるという契約は置かない。ホストは通知の bookKey/contentHash/sessionId と現在の認証ユーザーを照合し、ユーザー ID をクライアントの申告だけで信用しない。

## 7. 確認済み範囲と残るホスト受け入れ

### Bibi 側の既存検証（develop 時点）

Bibi の配布 JS/CSS のみを使った Chromium の 12 シナリオを確認済み: UTF-16 文字復元、フォント/resize、目次・型付き Jo 移動・実スクロール、明示初期位置と失敗、複数 iframe/source/session、user/file と同一書誌 ID の分離、拡張子なし入力と配置/preset 変更、lazy spine、挿絵と元 CFI、固定レイアウト、cookie 認証、署名相当の CORS/Range、atEnd と読み込み失敗。挿絵の確認は「右に縦長挿絵・左に本文の文字全体が表示され、resize 後も元の挿絵 CFI を維持する」を条件とし、次ページの端が少し見えただけでは成功としない。CI に同じ確認を追加した。

Windows の native Chromium と、固定 SHA の [Linux CI](https://github.com/Rumia-Channel/bibi/actions/runs/38291995676) の両方で成功した。Linux CI でも frozen install、39 unit tests、audit、配布物の build/manifest 検証、12 browser scenarios が成功している。develop の検証であり、2.1.0 Release の公開を意味しない。

指定された実 EPUB でも公開 API の ready、位置取得、離れた元 spine への復元と実際の本文表示を確認した。開発サーバーでも native Chromium で standalone ZIP Worker により固定レイアウト本が操作可能になった。unit suite は 39 件成功、audit は既知脆弱性 0。同一入力から作った完全 ZIP 2 本はバイト単位で一致し、別 timezone/ファイル時刻の再現性もテストする。

### 今回の Dantalian 取り込み準備の検証

- `27b05f9` のクリーンな checkout / Node 24.21.0 / Bun 1.4.2 で frozen install、39 unit tests、ビルド、manifest 検証、12 Chromium browser scenarios、ZIP 作成と `verify-zip` に成功。reader の検証対象は 17 ファイル。
- Dantalian の取り込み検証テストは 6 件成功。reader 改変・誤った pin・欠落/余分なファイル・危険なパス・重複・ホスト追加分の改変を拒否する。実際に upstream 配布物を改変して importer が失敗し、既存配布物が変わらないことも確認した。
- Native のビルドと EPUB Range 応答の回帰テスト 2 件に成功。3,303 バイトの実 EPUB に対する `bytes=-65557` は修正前 `416`、修正後 `206 / bytes 0-3302/3303` で、本文全体が元ファイルと一致した。通常の suffix Range も部分応答を維持する。
- Worker の `build_assets.mjs` と `worker-build --release` に成功。Native の配置先と Worker の `public/vendor/bibi/` はどちらも `--verify` に成功した。
- 実 Native の拡張子なし `/epubs/{hash}` と実ローカル Worker の ASSETS 上の `.epub` を、配布版 Jo・独自 preset で Firefox から開いた。両方で ready、本文表示、CFI `:4500` への復元後の可視位置 `:4300`、`spine-text` 進捗と identity 付き確定通知を確認した。別 contentHash の復元は `IDENTITY_MISMATCH` となり位置が変わらず、EPUB 内スクリプトは実行されなかった。Native の再確認には修正前の応答キャッシュを避ける book URL を使った。
- Windows の既定 MSVC C コンパイラーは依存ビルドで `D8050` を返したため、検証プロセスだけ `CC/CXX` にインストール済み `clang-cl` を指定した。Range テストの一時領域もプロセス限定で writable な `TEMP/TMP/SystemTemp` を指定した。リポジトリ・ユーザーのコンパイラー設定は変更していない。

Worker の確認は静的 ASSETS 配信であり、Wasabi の `/epubs/` 経路や認可を検証したものではない。`v0.1.15` の [Dantalian Linux CI](https://github.com/Rumia-Channel/Dantalian/actions/runs/38426015882) は Bibi のビルド・39 unit tests・12 browser scenarios まで成功したが、取り込みで lockfile checksum mismatch になり、Native/Worker と本番デプロイは実行されなかった。

`v0.1.16` では Windows checkout の CRLF ではなく Git の正規 LF 内容から `bun.lock` と `LICENSE` の固定チェックサムを取り直した。ホスト preset も `.gitattributes` で LF に固定する。正規 checkout で frozen install・ビルド・39 unit tests・12 browser scenarios・17 reader assets の取り込みと厳格な再検証・取り込み検証テスト 6 件が成功した。チェックサム検証を無効化した修正ではない。

### standalone 専用ページへの切替の検証

- `v0.1.17` の Native と Worker release Wasm build が成功した。Bibi の upstream 17 ファイルは変更せず、standalone 用 preset を取り込み直して厳格な検証を通した。
- 使い捨て Native DB に既存 API で 3,355 バイトの EPUB を登録し、本一覧の実カードから Bibi の専用ページへ移動した。`window === window.top`、window 直下の `Bibi.API`、ready/operable、本文の実表示、一覧ヘッダーと外側の reader iframe が存在しないことを確認した。
- キーボードのページ送りで `spine-text` 進捗が `0` から約 `0.2251` に変わり、CFI `/6/2!/4/2/4/1:9789` を取得した。ブラウザの「戻る」で直接本一覧に戻り、同じカードから開き直すと `initialRestore.ok:true`、同じ CFI・進捗を再開した。EPUB 内のテスト用スクリプトは実行されなかった。
- Native テスト 102 件成功・外部サービス依存 1 件除外、Worker helper テスト 28 件成功、取り込み検証 6 件成功。実 Native HTTP のデータ整合性契約 5 件も成功した。
- 認証必須の実ローカル Worker で、ヘッダーなしの起動ページから共通 API ログインを行い、書籍取得 `200` と EPUB 未登録案内を確認した。外部 URL を `from` に渡しても戻り先は本一覧になり、戻った本一覧では「認証設定」と正常な API 取得を確認した。Worker の auth・license・UI・整合性 HTTP 契約 9 件が成功した。これはローカルで Wasabi の EPUB を開いた検証ではない。

### 未実施の最終ホスト受け入れ

Dantalian の最終受け入れでは追加で次を実施する:

1. 固定 SHA のクリーンな成果物・独自 preset のみで standalone 専用ページを起動する。本番の登録済み EPUB でも、起動・ページ移動・再開を確認。
2. 実際の Native 認証 endpoint と Worker → Wasabi の最終 CORS/Range/署名期限を確認。
3. ホスト保存 API でサーバー位置を保存し、別端末で同じ user/book/file に復元。
4. user/book/file の切替、古いイベント/保留書き込み、競合、復元不能、ロード失敗を確認。
5. 画面幅・フォント・挿絵・目次・scroll を変えても同じ本文位置を再開できることを実画面で確認。
6. 末尾ジャンプだけでは読了にならず、明示的な読了操作が一覧に反映されることを確認。
7. 本番 CSP と EPUB 内スクリプト無効の設定で画面・console・ネットワークを確認。

上記の実施済み範囲と、サーバー位置保存・端末間同期・署名失効後の受け入れは別である。
