{
  __inputs = {
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  __functor =
    _:
    {
      pkgs,
      rust-overlay,
      rootSrc,
      self',
      ...
    }:
    let
      rustToolchain = pkgs.rust-bin.fromRustupToolchainFile (rootSrc + "/rust-toolchain.toml");
    in
    let
      lint-summary = pkgs.writeShellScriptBin "lint" ''
        ${pkgs.ast-grep}/bin/ast-grep scan --json=stream 2>/dev/null | ${pkgs.jq}/bin/jq -s -r '
          sort_by(.ruleId, .message, .note // "", .severity)
          | group_by([.ruleId, .message, .note // "", .severity])
          | .[]
          | "\(.[0].severity | ascii_upcase): \(.[0].ruleId) - \(.[0].message)\n"
            + (if (.[0].note? and .[0].note != "") then "NOTE: \(.[0].note)\n" else "" end)
            + (map("  - \(.file):\(.range.start.line+1):\(.range.start.column+1) - \(.text)") | join("\n"))
            + "\n"
        '
      '';
    in
    {
      default = pkgs.mkShell {
        packages = [
          rustToolchain
          pkgs.cargo-watch
          pkgs.cargo-edit
          pkgs.cargo-insta
          pkgs.rust-analyzer
          pkgs.pkg-config
          pkgs.openssl
          pkgs.ast-grep
          pkgs.yq-go
          pkgs.sccache
          pkgs.mold
          pkgs.clang
          lint-summary
          self'.formatter
        ];

        env = {
          RUSTC_WRAPPER = "sccache";
        };

        shellHook = ''
          if [ -t 0 ]; then
            if [ -d .git ]; then
              cp ${rootSrc}/nix/scripts/pre-commit .git/hooks/pre-commit
              chmod +x .git/hooks/pre-commit
            fi

            echo "Rust dev shell"
            echo "  Rust: $(rustc --version)"
            echo "  Cargo: $(cargo --version)"
            echo "  sccache: $(sccache --version)"
            echo ""
            echo "Available commands:"
            echo "  lint          - Run consolidated lint summary"
            echo "  sccache -s    - Show cache statistics"
          fi
        '';
      };
    };
}
