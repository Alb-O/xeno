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
      ...
    }:
    let
      rustToolchain = pkgs.rust-bin.fromRustupToolchainFile (rootSrc + "/rust-toolchain.toml");
      rustPlatform = pkgs.makeRustPlatform {
        cargo = rustToolchain;
        rustc = rustToolchain;
      };
    in
    {
      default = rustPlatform.buildRustPackage {
        pname = "evil";
        version = "0.1.0";
        src = rootSrc;
        cargoLock.lockFile = rootSrc + "/Cargo.lock";
        buildAndTestSubdir = "crates/term";
        # Integration tests require kitty terminal and CARGO_BIN_EXE_* env vars
        # that aren't available in Nix sandbox builds
        doCheck = false;
      };

      evildoer-core = rustPlatform.buildRustPackage {
        pname = "evildoer-core";
        version = "0.1.0";
        src = rootSrc;
        cargoLock.lockFile = rootSrc + "/Cargo.lock";
        buildAndTestSubdir = "crates/evildoer-core";
      };
    };
}
