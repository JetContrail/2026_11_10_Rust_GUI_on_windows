# `.paint` プロジェクトファイル形式 (version 1)

関連 Issue: #3, #8

- UTF-8 の JSON。人が読めて差分確認しやすく、`version` により将来の拡張と後方互換を保つ
- 下絵画像は PNG にエンコードし Base64 で埋め込む (単一ファイルで完結。#1 Q5 の承認内容に従う)

```json
{
  "format": "rust-paint-project",
  "version": 1,
  "canvas": { "width": 1280, "height": 720 },
  "layers": [
    {
      "id": 0, "name": "下絵", "visible": true, "locked": true,
      "content": { "type": "raster", "png_base64": "iVBORw0KGgo..." }
    },
    {
      "id": 1, "name": "描画", "visible": true, "locked": false,
      "content": {
        "type": "vector",
        "ops": [
          { "type": "stroke", "color": [255, 0, 0, 255], "width": 4, "points": [[10.5, 20.0], [30.0, 40.0]] },
          { "type": "erase", "width": 16, "points": [[12.0, 22.0]] },
          { "type": "clear_rect", "x": 0, "y": 0, "width": 100, "height": 50 },
          { "type": "text", "text": "こんにちは\nHello", "origin": [40.0, 60.0], "size": 32, "color": [0, 0, 0, 255], "font": "Yu Gothic UI" }
        ]
      }
    }
  ]
}
```

| フィールド | 内容 |
|---|---|
| `format` | 固定文字列。異なるファイルの誤読込を防ぐ |
| `version` | 形式バージョン。読込側は未知の新しい version をエラーにする |
| `canvas` | キャンバスサイズ (px) |
| `layers` | 下から順のレイヤー一覧 (レイヤー情報: id / 名前 / 表示 / 編集禁止) |
| `content.type = raster` | 下絵。`png_base64` は省略可 (下絵なし) |
| `content.type = vector` | 描画内容。`ops` は描画順 |
| `color` | RGBA 各 0-255 (色情報) |
| `width` | 線幅 / 消しゴムサイズ (px) |
| `text` | 文字列 (UTF-8、改行は `\n`)。空文字は不可 |
| `origin` | テキストの左上座標 (px) |
| `size` | 文字サイズ (px、8〜400) |
| `font` | フォントファミリー名。読込先の PC に無い場合や、そのフォントに無い文字は代替フォントで描画する |

読込時の検証: `format` / `version` の一致、キャンバスサイズの上限、下絵サイズとキャンバスサイズの整合、座標が有限値であること。不正な場合はエラーを返し、現在のドキュメントは変更しない。
