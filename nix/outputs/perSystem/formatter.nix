{
  __inputs = {
    treefmt-nix.url = "github:numtide/treefmt-nix";
    treefmt-nix.inputs.nixpkgs.follows = "nixpkgs";
  };

  __functor =
    _:
    {
      pkgs,
      treefmt-nix,
      imp-fmt-lib,
      rootSrc,
      ...
    }:
    imp-fmt-lib.mk {
      inherit pkgs treefmt-nix;
      excludes = [
        "target/*"
        "**/target/*"
        "vendor/*"
      ];
      rust = {
        enable = true;
        package = pkgs.rust-bin.fromRustupToolchainFile (rootSrc + "/rust-toolchain.toml");
      };
      kdlfmt = true;
    };
}
