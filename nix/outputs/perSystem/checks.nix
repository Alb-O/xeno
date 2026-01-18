{
  __inputs = {
    treefmt-nix.url = "github:numtide/treefmt-nix";
    treefmt-nix.inputs.nixpkgs.follows = "nixpkgs";

    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  __functor =
    _:
    {
      pkgs,
      self,
      self',
      treefmt-nix,
      imp-fmt-lib,
      rootSrc,
      ...
    }:
    let
      formatterEval = imp-fmt-lib.makeEval {
        inherit pkgs treefmt-nix;
        excludes = [
          "target/*"
          "**/target/*"
          "vendor/*"
        ];
        rust = true;
      };
    in
    {
      formatting = formatterEval.config.build.check self;

      ast-grep-scan = pkgs.runCommand "ast-grep-scan" { } ''
        cd ${rootSrc}
        ${pkgs.ast-grep}/bin/ast-grep scan
        touch $out
      '';

      # Package build implicitly runs tests via doCheck
      build = self'.packages.default;
    };
}
