{ mkCommandRule, ... }:
mkCommandRule {
  id = "no-raw-doc-locks";
  severity = "error";
  message = "Raw document lock access is forbidden; use Buffer::with_doc or DocumentHandle";
  run = ''
    rg --vimgrep "\\.read\\(\\)|\\.write\\(\\)" crates/editor/src \
      --glob '!crates/editor/src/buffer/mod.rs'
  '';
}
