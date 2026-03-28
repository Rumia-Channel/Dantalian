# Project Guidelines

## ライブラリ追加ルール

- **新しいライブラリ（クレート）を追加するときは、`Cargo.toml` に直接書き込まないこと。**
- 必ず `cargo add <crate-name>` コマンドを使用して追加すること。
- バージョン指定が必要な場合は `cargo add <crate-name>@<version>` を使用すること。
- features を指定する場合は `cargo add <crate-name> --features <feature>` を使用すること。
