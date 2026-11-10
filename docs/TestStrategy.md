# テスト戦略

関連 Issue: #3, #9, #11

| 種別 | 対象 | 場所 | 実行環境 |
|---|---|---|---|
| Unit Test | 描画ロジック (engine)、ツール (tools)、Undo/Redo (domain)、保存/読込 (persistence) | 各 crate の `#[cfg(test)]` | CI: windows-latest / ubuntu-latest |
| Integration Test | GUI を除く全コンポーネント (ViewModel + Tools + Engine + Domain + Persistence を実ファイル I/O で結合) | `crates/paint-app/tests/` | 同上 |
| E2E Test | アプリ全体の UI に実際のマウス/キー入力を与え、起動・描画・保存・再読込・Undo・Redo を確認 | `crates/paint-ui/tests/` (`egui_kittest`) | windows-latest |
| スモークテスト | リリース exe を実起動し、起動時間 (3 秒以内)・メモリ (500MB 以下) を計測 | `scripts/smoke.ps1` | windows-latest |

方針:
- すべての PR にテストを追加する。バグ修正には再現テストを先に追加する
- 描画結果はピクセル値で検証する (特定座標の色・アルファ、線幅による塗り範囲)
- ファイル系テストは一時ディレクトリを使用し、テスト間で状態を共有しない
