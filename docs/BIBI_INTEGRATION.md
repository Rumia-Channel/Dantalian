# Bibi 組み込み仕様と Dantalian 側の追加・接続作業

## 対象バージョンと役割

Bibi **2.1.0** のホスト連携機能を使用する。取り込み対象は [Rumia-Channel/bibi の commit `2ade2106f112bd9310f68c02b5102139dc4e7137`](https://github.com/Rumia-Channel/bibi/commit/2ade2106f112bd9310f68c02b5102139dc4e7137)。`develop` の移動する先端ではなく、この SHA を固定する。既存の `v2.0.0` タグには本書の新 API は含まれない。2.1.0 の master 統合・タグ作成・Release 公開とは別の変更である。

- **Bibi**: EPUB の取得・展開・描画、目次・移動、操作可能状態、元本文に対応した位置、進捗、端末内 Automark、公開 API と配布物検証。
- **Dantalian**: 認証・認可、書籍 ID とファイル版の管理、サーバー保存・同期、競合解決、現在位置と最遠位置、明示的な読了状態、ライブラリ一覧表示。
- この Dantalian リポジトリへの変更は**文書のみ**。以下の接続・追加項目をこの文書によって実装済みと見なしてはならない。作業中のアプリコードとも照合して組み込む。Bibi は Dantalian の保存 API を直接呼ばない。

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

クリーンな別 checkout で固定 ref と `bun.lock` を使用する。ローカルの作業中ディレクトリや古い `__dist` をそのままコピーしない。

```sh
git clone https://github.com/Rumia-Channel/bibi.git bibi-import
cd bibi-import
git checkout --detach 2ade2106f112bd9310f68c02b5102139dc4e7137
bun install --frozen-lockfile
bun run build
bun run verify:dist
bunx playwright install chromium
bun run test:browser
bun run archive
```

確認環境は Bun 1.4.2 / Node.js 24.21.0。再現性を比較する場合はこれらも同じバージョンに固定する。CI/Linux では Chromium のインストールに `bunx playwright install --with-deps chromium` を使う。

取り込むのは `__dist/bibi/` 全体、または ZIP の `Bibi-v2.1.0/bibi/` 全体。配下の相対関係を維持し `/vendor/bibi/` で配信する。デモ・サンプル本は不要。Jo だけ、reader JS だけを抜き出す配布は不可。

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

## 2. EPUB 配信と preset

### 同一 origin を既定にする

ホストと `/vendor/bibi/` は同じ origin で配信する。`book` には `/epubs/{file_hash}` を指定できる。URL は `URLSearchParams` で構築し、API の bearer token やユーザーの認証情報を query に載せない。

拡張子のない EPUB/ZIP endpoint は、ホスト側で `/vendor/bibi/presets/dantalian.js` を追加して次の設定を配信し、reader URL に `preset=dantalian` を指定する:

```javascript
Bibi.preset({
    'extract-if-necessary': ['.epub', '.zip', ''],
    'autostart': 'yes',
    'autostart-embedded': 'yes',
    'resume-from-last-position': 'yes',
    'keep-settings': 'yes',
    'allow-scripts-in-content': false,
    'request-with-credentials': false
});
```

`request-with-credentials:false` は**すべての cookie を禁止する意味ではない**。ZIP Worker/全体取得は `credentials:'same-origin'` を使用し、同一 origin の認証 cookie は送る。クロス origin には送らない。`true` の場合だけ `include` を使用する。Native endpoint の認証・認可はサーバー側で行い、未認証に EPUB 内容を返さない。

### Worker → Wasabi のリダイレクト

Worker の `/epubs/{file_hash}` が署名付き Wasabi URL にリダイレクトする構成では、最終ストレージ応答の CORS と Range を確認する。Worker 自体が EPUB 全体を直接配信する前提ではない。

- 同一 origin の開始 endpoint は cookie を受ける。`same-origin` のままの別 origin リダイレクト先にはホストの cookie を送らない。
- 必要な `Access-Control-Allow-Origin`、必要なら Range の OPTIONS、`Accept-Ranges: bytes`、正しい `206` / `Content-Range` / `Content-Length` を返す。
- JS が Range 情報を読めるよう `Access-Control-Expose-Headers: Content-Range, Accept-Ranges, Content-Length` を設定する。
- 資格情報なしの署名付きストレージ応答では `Allow-Origin:*` を使用できる。credentialed CORS では exact origin と `Allow-Credentials:true` が必要で、`*` を使わない。
- 署名の期限が複数 Range 要求を完了するまで有効か、期限切れ時にホストがどう再取得・位置復元するかを決める。署名 URL は短命の capability であり、長期認証 token を reader URL に渡す代替ではない。
- ローカルの cookie 認証・別 origin 署名相当リダイレクト・CORS/Range は確認済み。**実際の Dantalian Worker/Wasabi 設定での動作は未検証**で、統合時の受け入れ条件として残る。

### origin / CSP / iframe

- Jo は iframe の実際の `contentWindow` と reader origin を照合する。手書きの document-wide message 再配信を追加しない。
- 別 origin の親を許可する場合は、ホスト管理 preset の `trusted-origins` を exact origin の必要最小限にする。同一 origin 構成を先に検証する。
- `allow-scripts-in-content:false` を維持する。reader 自身の JavaScript と EPUB 内のスクリプトは別であり、前者まで無効化すると描画できない。
- CSP は reader/Jo の script、ZIP Worker と WASM、`blob:` の本文 iframe、本文 styles、画像・フォント、book の取得先を許可する必要がある。少なくとも `worker-src` / `frame-src` / `connect-src` / `img-src` / `font-src` を検討し、WASM はブラウザの `wasm-unsafe-eval` 要件も確認する。任意の `script-src *` や EPUB script 許可で回避しない。
- reader に iframe sandbox を付けるなら script と same-origin 権限が必要。`allow-scripts allow-same-origin` は同一 origin の信頼済み reader を隔離する防御境界にはならない。sandbox/CSP の最終方針はホスト側で実画面・console とともに受け入れ検証する。

## 3. 公開 API の使い方

ホストで `/vendor/bibi/and/jo.js` を読み込む。以下の関数の `Identity` と `InitialLocation` は、認証済みホストが書籍・ファイル・ユーザーに対応させた値を渡す。位置保存そのものを実装するサンプルではなく、実装済み Bibi API の呼び出し例である。

```javascript
async function openBibi(Container, Identity, InitialLocation) {
    const URLForReader = new URL('/vendor/bibi/', location.origin);
    URLForReader.search = new URLSearchParams({
        book: '/epubs/' + Identity.contentHash,
        preset: 'dantalian',
        'book-key': Identity.bookKey,
        'content-hash': Identity.contentHash,
        'storage-namespace': 'dantalian',
        'storage-user': Identity.userKey
    });
    if(InitialLocation !== undefined) {
        URLForReader.searchParams.set('location', JSON.stringify(InitialLocation));
    }
    const Bucket = new window['bibi:jo'].Bucket({ 'bibi-href': URLForReader.href });
    Bucket.api.subscribe('bibi:position-changed', Event => {
        console.log(Event.schemaVersion, Event.sessionId,
            Event.bookKey, Event.contentHash, Event.position);
    });
    Bucket.api.subscribe('bibi:state-changed', Event => {
        console.log(Event.state.status, Event.state.operable, Event.state.initialRestore);
    });
    Container.append(Bucket.Anchor);
    Bucket.embed();
    await Bucket.api.ready;
    return Bucket;
}
```

位置取得と実行中の復元:

```javascript
const State = await Bucket.api.getState();
const Position = await Bucket.api.getPosition();
const Result = await Bucket.api.restorePosition(Position.location, {
    bookKey: State.bookKey,
    contentHash: State.contentHash
});
console.log(Result.requestedLocation, Result.position);
```

`Bucket` は `openBibi` の戻り値を使う。保存レコードを復元するときは、**現在の State から作り直した識別子ではなく保存レコードの bookKey/contentHash** を `restorePosition` の第 2 引数に渡す。異なる本・ファイル版なら `IDENTITY_MISMATCH` となり移動しない。

画面を置き換える前は `Bucket.api.dispose()` を呼び、`Bucket.Frame` と `Bucket.Anchor` を DOM から外し、ホストが保持した参照も捨てる。dispose は購読と保留中の要求を解放する。本の切り替えは新しい Bucket で行う。直接同一 window で使用する場合は `Bibi.API` も同じ位置・状態契約を持つが、`getState/getPosition` は同期、`restorePosition` は非同期になる。内部の `R` / `E` / Biscuit に依存しない。

### schemaVersion 1 の契約

| API / フィールド | 意味 |
| --- | --- |
| `api.ready` | 操作可能になった時の Jo Promise。読み込み失敗は reject。 |
| `getState()` | schemaVersion、sessionId、bookKey、contentHash、status、operable、error、initialRestore。status は loading/ready/error。 |
| `getPosition()` | location、progress、progressBasis、atEnd、layout。|
| `restorePosition(location, expectedIdentity?)` | lazy spine を必要に応じロードして復元し、requestedLocation と実際の position を返す。 |
| `subscribe(type, listener)` | state-changed / position-changed の envelope。購読解除関数を返す。 |
| `position.layout` | iipp/pageIndex/pageCount。診断値でありサーバーの復元キーではない。 |

通知の位置は `Event.position.location` にある。参考段階の「envelope 直下の location」という提案形式ではない。schemaVersion/sessionId/bookKey/contentHash は envelope に付く。要求応答は requestId に対応し、Jo がこの protocol を処理する。

`ready` が一度 resolve しても、resize・表示モード変更・復元の途中は `state.operable:false` になり得る。必要なら state-changed で再び操作可能になったことを待つ。失敗は `Error.code` で扱う: NOT_READY、BUSY、INVALID_LOCATION、LOCATION_NOT_FOUND、INVALID_IDENTITY、IDENTITY_MISMATCH、RESTORE_FAILED、LOAD_FAILED。Jo には TIMEOUT、SESSION_CHANGED、DISPOSED もある。失敗を「保存位置がないので成功」と扱わず UI に状態を残す。

## 4. 保存する位置・進捗・終端

### 元ソースの位置

`position.location` は `{ p, cfi }`。推奨復元キーは**文字 offset を含む cfi**で、必要なら p も一緒に保存する。元の sanitization 後の DOM を pagination/挿絵移動より前に記録するため、表示用 DOM の nth-child をホストが取り直す必要はない。

- 対応形式: linear spine の `/6/<偶数>!/4/...` point CFI と UTF-16 文字 offset。長い段落の途中に復元できる。P は original linear item と element の位置で、段落内の文字位置までは表さない。
- 両方あれば CFI 優先。無効な CFI を P にすり替えたり範囲外を末尾に clamp したりしない。
- range CFI、nested indirect CFI、nonlinear spine、存在しないノード・文字 offset は対象外/失敗。CFI 全仕様への対応を意味しない。
- lazy item は live restore 時にロードしてから解決する。挿絵が隣接本文へ移されても元の spine の CFI を維持する。
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

本節はアプリ実装の接続仕様。今回アプリコードは変更していないため、既存機能・作業中変更との重複を確認しながら実装する。

| 優先度 | 追加・接続対象 | 完了条件 |
| --- | --- | --- |
| P0 | 固定 SHA/lock/toolchain の成果物取り込み、独自 preset と静的配信 | manifest と Jo/Worker を検証した配布物だけで `/vendor/bibi/` の画面が動く。 |
| P0 | 認証済み user/book/file の reader 起動と切替 | 身元はサーバー側で確定し、他ユーザーのファイル・位置を取得できない。旧 Bucket/保存要求を次の書籍へ流用しない。 |
| P0 | 位置保存 DB と認可済み取得/更新 API | user + book + contentHash + schemaVersion + location + progressBasis/progress + 更新 version/時刻を管理。ファイル更新前の CFI を新ファイルに無条件適用しない。 |
| P0 | 初期サーバー位置取得と live restore、失敗表示 | 保存済み identity を確認して明示初期位置を渡す。復元不能・ロード失敗を表示し、無言で古い/別の本の位置を保存しない。 |
| P1 | 確定位置イベントの保存キュー | 操作中/BUSY を除き debounce・最後の位置を扱う。離脱時の flush と再開・オフライン時の扱い、旧 session 要求の失効を定義する。 |
| P1 | 現在位置・最遠到達位置・明示読了を分離 | 後戻りしても現在位置を保存。furthest は別の値、finished は明示操作等のポリシーで更新。atEnd だけでは finished にしない。 |
| P1 | 端末間・複数タブの競合とファイル版更新 | サーバー version/ETag 等で競合を検出し、古い端末の遅延書き込みを判別。別 contentHash は別位置として扱う。 |
| P2 | ライブラリ一覧と再開 UX | 未読/読書中/読了、現在・最遠進捗、最後に読んだ時刻、復元失敗/ファイル更新の状態を表示。 |

保存 API の endpoint 名や DB カラムは Dantalian の既存設計に合わせて決める。本書の Bibi API と異なり、未作成 endpoint を呼べるという契約は置かない。ホストは通知の bookKey/contentHash/sessionId と現在の認証ユーザーを照合し、ユーザー ID をクライアントの申告だけで信用しない。

## 7. 確認済み範囲と残るホスト受け入れ

Bibi の配布 JS/CSS のみを使った Chromium の 12 シナリオを確認済み: UTF-16 文字復元、フォント/resize、目次・型付き Jo 移動・実スクロール、明示初期位置と失敗、複数 iframe/source/session、user/file と同一書誌 ID の分離、拡張子なし入力と配置/preset 変更、lazy spine、挿絵と元 CFI、固定レイアウト、cookie 認証、署名相当の CORS/Range、atEnd と読み込み失敗。CI に同じ確認を追加した。

指定された実 EPUB でも公開 API の ready、位置取得、離れた元 spine への復元と実際の本文表示を確認した。開発サーバーでも native Chromium で standalone ZIP Worker により固定レイアウト本が操作可能になった。unit suite は 39 件成功、audit は既知脆弱性 0。同一入力から作った完全 ZIP 2 本はバイト単位で一致し、別 timezone/ファイル時刻の再現性もテストする。

Dantalian の最終受け入れでは追加で次を実施する:

1. 固定 SHA のクリーンな成果物・独自 preset のみを配置して Jo から起動。
2. 実際の Native 認証 endpoint と Worker → Wasabi の最終 CORS/Range/署名期限を確認。
3. ホスト保存 API でサーバー位置を保存し、別端末で同じ user/book/file に復元。
4. user/book/file の切替、古いイベント/保留書き込み、競合、復元不能、ロード失敗を確認。
5. 画面幅・フォント・挿絵・目次・scroll を変えても同じ本文位置を再開できることを実画面で確認。
6. 末尾ジャンプだけでは読了にならず、明示的な読了操作が一覧に反映されることを確認。
7. 本番 CSP/sandbox とリーダー内スクリプト無効の設定で画面・console・ネットワークを確認。

実 Wasabi・Dantalian の位置保存/同期・本番 CSP を本書で検証済みとするものではない。
