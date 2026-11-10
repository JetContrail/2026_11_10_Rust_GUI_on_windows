# 開発計画 (Development Plan)

関連 Issue: #2 (リポジトリ解析・開発計画), #1 (仕様確認)

## 1. リポジトリ解析結果

| 項目 | 結果 |
|---|---|
| 初期状態 | 空リポジトリ (コミット 0 件、ブランチなし) |
| 対応 | `main` に bootstrap コミット (.gitignore / .gitattributes / README スタブ) のみ作成。以降は全て PR 経由 |
| ツールチェーン | Rust stable (`rust-toolchain.toml` で `channel = "stable"` を指定し常に最新安定版を使用) |
| 対象 | Windows 10 / 11 64bit (`x86_64-pc-windows-msvc`)、静的 CRT リンクで単体実行可能な exe |

## 2. GUI フレームワーク選定

| 観点 | egui / eframe | iced |
|---|---|---|
| 保守性 | 即時モード。状態は ViewModel が一元管理し、UI は状態を描くだけなので MVVM と相性が良い | Elm 型。メッセージ設計が必要で、キャンバス描画のカスタムウィジェット実装が重い |
| キャンバス実装 | `Painter` / テクスチャ部分更新 (`set_partial`) が標準で使える | `canvas` widget はベクタ中心。大きなラスタの部分更新は不得手 |
| テスト | `egui_kittest` で UI に実入力を与える E2E が可能 | 公式の UI テストハーネスが弱い |
| リリース頻度・利用者 | 活発 (0.37, 2026-10) | 活発 |
| 日本語表示 | eframe 0.37 の `system_fonts` で OS フォント (DirectWrite) を自動フォールバック | 要フォント設定 |

**結論: eframe (アプリ枠) + egui (UI) を採用。** egui は UI ライブラリ、eframe はそのネイティブ実行フレームワークであり、要件の優先順位 1・2 を両方満たす。描画バックエンドは eframe 既定の wgpu (DirectX 12 / Vulkan、GPU 非搭載環境では WARP にフォールバック) を使用する。

## 3. 開発ステップと PR 対応

PR は前ステップのブランチをベースにした積み上げ (stacked) 方式。**マージは下から順に** 行う。

| Step | 内容 | Issue | 主な成果物 |
|---|---|---|---|
| 1-2 | リポジトリ解析・開発計画 | #2 | 本書 |
| 3 | GitHub Issues 作成 | — | #1〜#13 |
| 4 | システム設計 | #3 | `docs/Architecture.md` / `docs/FileFormat.md` / `docs/TestStrategy.md` |
| 5 | Rust プロジェクト構築 + 最小 CI | #4 | Cargo workspace、`.github/workflows/ci.yml` |
| 6-1 | Domain + Undo/Redo | #5 | `crates/paint-domain` |
| 6-2 | Drawing Engine | #6 | `crates/paint-engine` |
| 6-3 | Tool Logic | #7 | `crates/paint-tools` |
| 6-4 | Persistence | #8 | `crates/paint-persistence` |
| 6-5 | ViewModel + Integration Test | #9 | `crates/paint-app` |
| 6-6 | UI | #10 | `crates/paint-ui` (実行ファイル `paint.exe`) |
| 7-8 | E2E テスト + GitHub Actions 完成 | #11 | E2E (Windows)、起動時間/メモリ計測、リリース成果物 |
| 9 | ドキュメント整備 | #12 | README、利用者マニュアル、rustdoc 自動生成 |
| 10-11 | PR 作成・成果物レビュー報告 | #13 | `docs/ReviewReport.md` |

CI はステップ 5 で最小構成 (fmt / clippy / test / build --release) を先に導入し、以降の全 PR を自動検証する。ステップ 7-8 で E2E・計測・成果物アップロードを追加して完成させる。

## 4. 開発ルール

- GitHub Flow: `main` は常にビルド可能。変更は `devin/<timestamp>-<slug>` ブランチ → PR → PO 承認 → マージ
- Issue 駆動: すべての PR は対応 Issue を `Refs #N` / `Closes #N` で参照
- 全変更にテストを追加。CI (fmt / clippy `-D warnings` / test / release build) が緑であること
- PR 本文に「変更概要 / レビュー観点 / テスト結果 / 残課題 / マージ提案」を記載
- 仕様の不明点は推測で実装せず Issue 化 (`spec-question` ラベル) し、PO 承認後に実装

## 5. 品質目標と確認方法

| 要件 | 目標 | 確認方法 |
|---|---|---|
| 起動時間 | 3 秒以内 | Windows CI でリリース exe を起動し、ウィンドウ表示までの時間を計測 |
| メモリ | 500MB 以下 | 同スモークテストでワーキングセットを計測。画像サイズ上限 (#1 Q13) で最大値を抑制 |
| クラッシュゼロ | `unwrap` / `panic` を本番コードで使用しない方針 (clippy で `unwrap_used` を警告)。ファイル I/O は全て `Result` でエラー表示 |
| Undo/Redo | 各 50 回以上 | Unit Test / Integration Test / E2E |

## 6. リスク

| リスク | 対策 |
|---|---|
| CI ランナーに GPU がない | wgpu の WARP (ソフトウェア DirectX 12) で起動。E2E の主検証は GPU 不要な `egui_kittest` で実施 |
| 大きな画像でメモリ超過 | 画像サイズ上限 (#1 Q13)、Undo 履歴は差分 (描画操作) のみ保持 |
| 長時間描画で Undo 時の再描画が遅くなる | 描画操作をレイヤーキャッシュに逐次反映し、Undo 時のみ再構築。将来はチェックポイント方式に拡張可能 |
