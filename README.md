# dotstroke_for_pd

## Rust + egui版

Rust版のエディタは `src/main.rs` にあります。起動するにはRust toolchainを用意して、次を実行します。

```sh
cargo run
```

ズームはマウスホイールまたは左パネルのボタン、パンは中央ボタンのドラッグです。既存の `pdvector` JSONを読み込み、編集結果をJSONとして保存できます。

旧Tkinter版は比較用として `dotstroke.py` に残しています。
