# 本番デプロイチェックリスト（Cloudflare Workers / D1 / Wasabi）

本番環境（`v*` タグによるデプロイ）およびステージング環境への反映前後で確認するチェックリストです。

---

## 1. デプロイ前の確認

- [ ] `develop` の変更がレビュー済みであり、リリース候補として `master` へ `--no-ff` マージされている（`master` への push で staging、本番は `master` 上の `v*` タグのみでデプロイ）。
- [ ] バージョン番号が `python scripts/bump_version.py X.Y.Z` で更新され、旧バージョン文字列の残存がない。
- [ ] Native 環境から移行する場合、SQLite バックアップの取得とリストア確認が完了し、移行スナップショット中の Native 書き込みを停止している。
- [ ] 本番 D1 の移行前スナップショットについて、D1 Time Travel の復元用ブックマークが記録されている（復元は書き込み停止後に `wrangler d1 time-travel restore` を使用）。
- [ ] 対象の D1 データベースへ最新の `worker/migrations/` が適用済みである。
- [ ] Wasabi のバケット・リージョン・エンドポイント・環境別プレフィックス（staging: `dantalian/staging`、production: `dantalian/production`）を確認済み。
- [ ] D1 / Wasabi / トークン類はシークレットストアのみに格納し、ログやビルド成果物に平文出力されない。

---

## 2. Cloudflare Access / セキュリティ境界

- [ ] 本番カスタムドメイン（`SERVICE_DOMAIN`）が Cloudflare Access アプリケーションで保護され、明示的な許可ポリシーが設定されている。
- [ ] `workers.dev` ルートが無効化されており、未認証の `GET /api/health` がアプリへ到達しない（本番 CI が自動検証）。
- [ ] 署名付き URL 発行ルートはすべて Access 保護下のアプリ境界内からのみ到達可能である。
- [ ] `DANTALIAN_STAGING_API_TOKEN`（staging テスト用）は本番トークンと独立したランダム値になっている。

---

## 3. 外部音声プロセッサ（Queue / Container）

- [ ] GitHub Environment `production` の環境変数（Cloudflare API 認証情報、5 つの Wasabi `*_SECRET_NAME`、`DANTALIAN_D1_DATABASE_NAME=dantalian`、`DANTALIAN_AUDIO_JOB_QUEUE=dantalian-audio-jobs`、`SERVICE_DOMAIN`）が揃っている。
- [ ] `wrangler.audio.toml` を本番へ手動デプロイしていない（CI の描画済み本番設定を使用する）。
- [ ] 音声プロセッサ用の Wasabi アクセスキーには最小権限 IAM キーを使用し、ルートキーを使っていない。
- [ ] キューの ack がコンテナ起動後に返され、音声プロセッサのログに機密値や署名付き URL 全文が出力されない。
- [ ] テスト用の短い音声で Opus / AAC 変換ジョブが `queued → running → completed`（および失敗時リトライ）に遷移することを確認済み。

---

## 4. Wasabi バケット CORS 設定（ブラウザ直送用）

Worker 本番環境では全書籍ファイル（EPUB / PDF / ZIP）および 8 MiB 以上の音声をブラウザから Wasabi へ presigned multipart で直接アップロードし、再生・閲覧時は署名付き URL へ `302` リダイレクトします。Wasabi バケットに以下の CORS ルールを設定してください（未設定の場合、ブラウザ側で `net::ERR_FAILED` や ETag 取得不可エラーになります）。

- [ ] **Allowed Origins**: `https://<SERVICE_DOMAIN>` のみ（ワイルドカード `*` 不可）
- [ ] **Allowed Methods**: `PUT`, `GET`, `HEAD`
- [ ] **Allowed Headers**: 指定不要（クライアントはカスタムヘッダを送らないため単純リクエストになる）
- [ ] **Expose Headers**: `ETag`（multipart 完了通知で各パートの ETag 読み取りに必須）

---

## 5. デプロイ・スモーク確認

- [ ] CI の全チェック（`bibi` ビルド検証、`native` チェック・テスト、`worker` wasm ビルド・各種契約テスト、deployed E2E）が成功している。
- [ ] 静的アセット（`_headers`）のレスポンスに `X-Content-Type-Options` / `X-Frame-Options` / `Referrer-Policy` / `Content-Security-Policy` が付与されている。
- [ ] 書籍登録、カバー画像アップロード（上限 10 MiB、Worker 経由）、書籍ファイルアップロード（EPUB / PDF / ZIP、Wasabi 直送）、CD・8 MiB 以上の音声アップロード（Wasabi 直送 + ローカル WASM メタデータ解析）が成功し、Network パネルで `PUT` 応答の `ETag` と `/finalize` の `200` を確認できる。
- [ ] Bibi リーダーで書籍を開き、ブラウザコンソールに CSP 違反が出ない。
- [ ] 移行（`dantalian_migrate`）を実施した場合、reconciliation レポートで件数・チェックサムの不整合がない。切り戻しが必要になるまで Native 環境をロールバック用として保持する。

---

## 6. ロールバック手順

1. Worker への書き込みトラフィックを停止し、読み書きを Native（または復元した D1 Time Travel ポイント）へ切り戻す。
2. 移行ステート／レポートおよび機密情報をマスクしたログを保全する。
3. ロールバック中に共有 Wasabi バケット上のオブジェクトを削除しない（テスト用プレフィックスのみ個別調査後に削除）。
4. D1 とオブジェクトストレージの整合性を確認してから再実施する。
