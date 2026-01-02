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
    {
      default = pkgs.mkShell {
        # Compose with imp-lint devshell
        inputsFrom = [ self'.devShells.imp-lint ];

        packages = [
          rustToolchain
          pkgs.cargo-watch
          pkgs.cargo-edit
          pkgs.cargo-insta
          pkgs.rust-analyzer
          pkgs.pkg-config
          pkgs.openssl
          pkgs.mold
          pkgs.clang
          self'.formatter
        ];

        shellHook = ''
          if [ -t 0 ]; then
            echo ""
            echo "Rust dev shell"
            echo "  Rust: $(rustc --version)"
            echo "  Cargo: $(cargo --version)"
          fi
        '';
      };
    };
}
