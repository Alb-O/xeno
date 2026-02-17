# Changelog

## 2026-02-17

* Breaking: removed runtime compatibility wrappers `Editor::on_event` and `Editor::pump`.
* Frontend/runtime loop contract is now `submit_event` / `submit_external_event` + `drain_until_idle` + `poll_directive`.
* Frontend loop consumers should treat `LoopDirectiveV2` as the canonical runtime directive surface.
