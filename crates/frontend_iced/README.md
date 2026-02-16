# xeno-frontend-iced

Experimental `iced_wgpu` frontend for `xeno-editor`.

## Local dependency

This crate expects an `iced` checkout at `../iced` relative to the `xeno` workspace root.

If you keep a symlink at `xeno/iced -> ../iced`, the dependency path resolves correctly.

## Commands

```bash
cargo check -p xeno-frontend-iced --features iced-wgpu
cargo run -p xeno-frontend-iced --features iced-wgpu --bin xeno-iced -- [path] [--theme NAME]
```

## Linux backend selection

* Auto-selection prefers Wayland when `WAYLAND_DISPLAY` is present.
* If you need to force one backend, set `XENO_ICED_BACKEND` to `wayland` or `x11`.
* `WINIT_UNIX_BACKEND` still takes precedence if already set.

## Resize mapping

Core runtime resize events use text grid units (`cols`/`rows`).
The iced frontend maps logical pixels to cells using:

* `XENO_ICED_CELL_WIDTH_PX` (default `8`)
* `XENO_ICED_CELL_HEIGHT_PX` (default `16`)

## Layout tuning

* `XENO_ICED_INSPECTOR_WIDTH_PX` sets sidebar width in logical pixels (default `320`, minimum `160`).
* `XENO_ICED_SHOW_INSPECTOR` toggles the sidebar (`1`/`true`/unset = visible, `0`/`false`/`no`/`off` = hidden).
