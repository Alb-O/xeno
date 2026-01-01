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
      imp,
      ...
    }:
    let
      rustToolchain = pkgs.rust-bin.fromRustupToolchainFile (rootSrc + "/rust-toolchain.toml");

      # Collect fragments from .d directories (injected by gits)
      shellHookFragments = imp.fragments ./shellHook.d;
      packageFragments = imp.fragmentsWith { inherit pkgs self'; } ./packages.d;
    in
    {
      default = pkgs.mkShell {
        packages =
          [
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
          ]
          ++ packageFragments.asList;

        shellHook = ''
          ${shellHookFragments.asString}

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
