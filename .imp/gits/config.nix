{
  injections = [
    {
      name = "imp-lint";
      remote = "https://github.com/imp-nix/imp.lint.git";
      use = [
        # Lint rules (can customize per-project)
        "lint/rules"
        "lint/custom"
        # Scripts
        "nix/scripts"
        # Nix lib for rule helpers
        "nix/lib"
        # Output fragments
        "nix/outputs/perSystem/packages.d/10-lint.nix"
        "nix/outputs/perSystem/packages.d/20-lint-rules.nix"
        "nix/outputs/perSystem/devShells.d/10-imp-lint.nix"
      ];
    }
    {
      name = "rust-boilerplate";
      remote = "https://github.com/Alb-O/rust-boilerplate.git";
      use = [
        "rust-toolchain.toml"
        "rustfmt.toml"
        "clippy.toml"
        # formatter.nix kept local - needs kdlfmt
      ];
    }
  ];
}
