{ mkRule }:
mkRule {
  id = "no-decorative-headings";
  language = "rust";
  severity = "warning";
  message = "Move section comments into /// docstrings with # headers and examples";
  files = [ "**/*.rs" ];
  ignores = [ "**/target/**" ];
  rule = {
    all = [
      {
        kind = "line_comment";
        regex = ''^//\s*([=\-\*\+#_]){3,}.*$'';
      }
      {
        not = {
          regex = "^//[/!]";
        };
      }
    ];
  };
}
