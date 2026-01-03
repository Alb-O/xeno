{
  pkgs,
  lib,
  self,
  ...
}:
let
  astGrep = import "${self}/nix/lib/ast-grep-rule.nix" { inherit lib; };
  customRule = import "${self}/nix/lib/custom-rule.nix" { inherit lib; };

  # ast-grep rules from lint/rules/
  rulesDir = "${self}/lint/rules";
  ruleFiles = builtins.filter (f: lib.hasSuffix ".nix" f) (
    builtins.attrNames (builtins.readDir rulesDir)
  );
  rules = map (f: {
    name = lib.removeSuffix ".nix" f;
    rule = import (rulesDir + "/${f}") { inherit (astGrep) mkRule; };
  }) ruleFiles;

  # Generate YAML files
  generatedRules = pkgs.runCommand "ast-grep-rules" { buildInputs = [ pkgs.yq-go ]; } ''
    mkdir -p $out
    ${lib.concatMapStringsSep "\n" (
      r: ''echo '${astGrep.toJson r.rule}' | yq -P > $out/${r.name}.yml''
    ) rules}
  '';

  # custom rules from lint/custom/
  customRulesDir = "${self}/lint/custom";
  customRuleFiles = builtins.filter (f: lib.hasSuffix ".nix" f) (
    builtins.attrNames (builtins.readDir customRulesDir)
  );
  customRules = map (f: import (customRulesDir + "/${f}") customRule) customRuleFiles;
  customRulesJson = builtins.toJSON customRules;

  # The module script
  moduleScript = "${self}/nix/scripts/imp-lint.nu";

  # Nushell module package (installed to $out/lib/imp-lint)
  impLintModule = pkgs.runCommand "imp-lint" { } ''
    mkdir -p $out/lib
    substitute ${moduleScript} $out/lib/imp-lint \
      --replace-warn '@impLintRules@' '${generatedRules}' \
      --replace-warn '@impLintRulesInjected@' 'true' \
      --replace-warn "@impLintCustomRules@" '${customRulesJson}' \
      --replace-warn '@impLintCustomRulesInjected@' 'true'
  '';
in
{
  imp-lint = impLintModule;
  default = impLintModule;
}
