/**
  Composable lintfra shell configuration.
  Provides packages and shellHook for lint tooling.

  Usage in consuming repo's devShells.nix:

  ```nix
  {
    default = pkgs.mkShell {
      inputsFrom = [ self'.devShells.lintfra ];
      packages = [ ... ];  # your packages
    };
  }
  ```
*/
{ pkgs, self', ... }:
{
  lintfra = pkgs.mkShell {
    packages = [
      pkgs.ast-grep
      pkgs.yq-go
      pkgs.jq
      self'.packages.lint
    ];

    shellHook = ''
      # Install pre-commit hook
      if [ -t 0 ] && [ -d .git ]; then
        if [ -x ./nix/scripts/pre-commit ]; then
          cp ./nix/scripts/pre-commit .git/hooks/pre-commit
          chmod +x .git/hooks/pre-commit
        fi
      fi

      echo "Lint commands:"
      echo "  lint        - Run unified lint (ast-grep + custom rules)"
      echo "  lint --json - Output lint results as JSON stream"
    '';
  };
}
