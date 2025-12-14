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
        packages = [
          rustToolchain
          pkgs.cargo-watch
          pkgs.cargo-edit
          pkgs.cargo-insta
          pkgs.rust-analyzer
          self'.formatter
        ];

        shellHook = ''
          echo "Rust dev shell"
          echo "  Rust: $(rustc --version)"
          echo "  Cargo: $(cargo --version)"
        '';
      };
    };
}
