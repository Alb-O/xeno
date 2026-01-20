{ mkRule }:
mkRule {
  id = "allow-attribute-reason";
  language = "rust";
  severity = "info";
  message = ''#[allow(...)] should include reason = "..." justification'';
  files = [ "**/*.rs" ];
  rule = {
    any = [
      {
        all = [
          { pattern = "#[allow($$$ARGS)]"; }
          {
            not = {
              regex = ''reason\s*='';
            };
          }
        ];
      }
      {
        all = [
          { pattern = "#![allow($$$ARGS)]"; }
          {
            not = {
              regex = ''reason\s*='';
            };
          }
        ];
      }
    ];
  };
}
