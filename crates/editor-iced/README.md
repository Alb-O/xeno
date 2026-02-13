# xeno-editor-iced

Experimental `iced_wgpu` frontend for `xeno-editor`.

## Local dependency

This crate expects an `iced` checkout at `../iced` relative to the `xeno` workspace root.

If you keep a symlink at `xeno/iced -> ../iced`, the dependency path resolves correctly.

## Commands

```bash
cargo check -p xeno-editor-iced --features iced-wgpu
cargo run -p xeno-editor-iced --features iced-wgpu --bin xeno-iced -- [path] [--theme NAME]
```
