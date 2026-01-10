# Rust packages for xeno
# Merged with other packages.d/ fragments (e.g., 10-lint.nix from lintfra)
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
      cargoToml = builtins.fromTOML (builtins.readFile (rootSrc + "/Cargo.toml"));
      version = cargoToml.workspace.package.version;
      rustToolchain = pkgs.rust-bin.fromRustupToolchainFile (rootSrc + "/rust-toolchain.toml");
      rustPlatform = pkgs.makeRustPlatform {
        cargo = rustToolchain;
        rustc = rustToolchain;
      };
    in
    {
      default = rustPlatform.buildRustPackage {
        pname = "xeno";
        inherit version;
        src = rootSrc;
        cargoLock.lockFile = rootSrc + "/Cargo.lock";
        buildAndTestSubdir = "crates/term";
        nativeBuildInputs = [ pkgs.clang pkgs.mold ];
        # Integration tests require kitty terminal and CARGO_BIN_EXE_* env vars
        # that aren't available in Nix sandbox builds
        doCheck = false;
        meta.mainProgram = "xeno";
      };

      xeno-core = rustPlatform.buildRustPackage {
        pname = "xeno-core";
        inherit version;
        src = rootSrc;
        cargoLock.lockFile = rootSrc + "/Cargo.lock";
        buildAndTestSubdir = "crates/xeno-core";
      };
    };
}
