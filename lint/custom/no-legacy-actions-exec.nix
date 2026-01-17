{ mkCommandRule, ... }:
mkCommandRule {
  id = "no-legacy-actions-exec";
  severity = "error";
  message = "Legacy actions_exec call sites are forbidden; use invocation";
  run = ''
    rg --vimgrep "actions_exec::" crates/editor/src \
      --glob '!crates/editor/src/impls/actions_exec.rs' \
      --glob '!crates/editor/src/impls/mod.rs'
  '';
}
