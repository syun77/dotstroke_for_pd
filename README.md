# dotstroke_for_pd

## Rust + egui版

Rust版のエディタは `src/main.rs` にあります。起動するにはRust toolchainを用意して、次を実行します。

```sh
cargo run
```

ズームはマウスホイールまたは左パネルのボタン、パンは中央ボタンのドラッグです。既存の `pdvector` JSONを読み込み、編集結果をJSONとして保存できます。

旧Tkinter版は比較用として `dotstroke.py` に残しています。

Style の Dither pattern から Playdate SDK のディザパターンを指定できます。選択ツールで既存ベクターを選択している場合は、そのベクターのパターンを変更します。Lua 出力では `gfx.setDitherPattern(0.5, gfx.image.kDitherType...)` として出力されます。

ファイル操作は File メニューから行えます。macOS ではシステムメニューバーにも表示され、`Cmd+N` で新規作成、`Cmd+O` で JSON 読み込み、`Cmd+S` で保存できます。読み込み済みの JSON は `Cmd+S` で同じファイルへ直接保存されます。
