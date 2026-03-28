# Project Guidelines

## ライブラリ追加ルール

- **新しいライブラリ（クレート）を追加するときは、`Cargo.toml` に直接書き込まないこと。**
- 必ず `cargo add <crate-name>` コマンドを使用して追加すること。
- バージョン指定が必要な場合は `cargo add <crate-name>@<version>` を使用すること。
- features を指定する場合は `cargo add <crate-name> --features <feature>` を使用すること。

## Web UI アセットのキャッシュバスタ

- `static/style.css` および `static/script.js` の変更をブラウザに即座に反映させるため、キャッシュバスタ（`?v=<hash>`）を使用している。
- `build.rs` が両ファイルの更新日時をハッシュ化し、`ASSET_VERSION` 環境変数にセットする。
- `static/index.html` 内の `?v=ASSET_VERSION` はコンパイル時に実際のハッシュ値に置換される（`src/main.rs` の `serve_index` ハンドラ経由）。
- CSS/JS を変更した場合は必ずリビルド（`cargo build`）すること。リビルドしないとハッシュが更新されず、ブラウザが古いキャッシュを使い続ける。
- 新しい静的アセット（CSS/JS ファイル）を追加した場合は、`build.rs` に `println!("cargo:rerun-if-changed=static/<file>");` を追加し、`index.html` にも `?v=ASSET_VERSION` を付与すること。
