{
  injections = [
    {
      name = "lintfra";
      remote = "https://github.com/Alb-O/lintfra.git";
      use = [
        "lint/ast-rules"
        "lint/custom-rules"
        "nix/scripts"
        "nix/outputs/perSystem/packages/lint.nix"
        "nix/outputs/perSystem/shellHook.d/10-lintfra.sh"
        "nix/outputs/perSystem/packages.d/10-lintfra.nix"
        "sgconfig.yml"
      ];
    }
  ];
}
