{ mkRule }:
mkRule {
  id = "no-short-comments";
  language = "rust";
  severity = "info";
  message = "Short comments often unnecessary; prefer self-explanatory code or docstrings";
  files = [ "**/*.rs" ];
  ignores = [
    "**/target/**"
    "**/tests/**/*.rs"
    "**/benches/**/*.rs"
  ];
  rule = {
    all = [
      {
        kind = "line_comment";
        regex = ''^//\s*.{0,25}\s*$'';
      }
      {
        not = {
          any = [
            { regex = "^//[/!]"; }
            {
              regex = ''^//\s*(?i)(TODO|FIXME|BUG|SAFETY|NOTE|CHECK|XXX|HACK|DEBUG|INTERNAL|TEST|STUB|WAIT|FALLTHROUGH|REFACTOR|OPTIMIZE|DOCS|FIX|PERF|WARN|WARNING|INFO|LINT|ANCHOR)'';
            }
            { regex = ''^//\s*(\.\.\.|$)''; }
          ];
        };
      }
    ];
  };
}
