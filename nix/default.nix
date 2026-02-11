inputs@{
  flake-parts,
  systems,
  rust-overlay,
  treefmt-nix,
  ...
}:
flake-parts.lib.mkFlake { inherit inputs; } {
  systems = import systems;

  imports = [ treefmt-nix.flakeModule ];

  perSystem =
    {
      config,
      pkgs,
      ...
    }:
    let
      rootSrc = ./..;
      cargoToml = builtins.fromTOML (builtins.readFile (rootSrc + "/Cargo.toml"));
      workspaceVersion = cargoToml.workspace.package.version;

      rustPkgs = pkgs.extend rust-overlay.overlays.default;
      rustToolchain = rustPkgs.rust-bin.fromRustupToolchainFile (rootSrc + "/rust-toolchain.toml");
      rustPlatform = pkgs.makeRustPlatform {
        cargo = rustToolchain;
        rustc = rustToolchain;
      };

      cargoSortWrapper = pkgs.writeShellScriptBin "cargo-sort-wrapper" ''
        set -euo pipefail

        opts=()
        files=()

        while [[ $# -gt 0 ]]; do
          case "$1" in
            --*) opts+=("$1"); shift ;;
            *) files+=("$1"); shift ;;
          esac
        done

        for f in "''${files[@]}"; do
          ${pkgs.lib.getExe pkgs.cargo-sort} "''${opts[@]}" "$(dirname "$f")"
        done
      '';

      rustPackage = rustPlatform.buildRustPackage {
        pname = "xeno";
        version = workspaceVersion;
        src = rootSrc;

        cargoLock = {
          lockFile = rootSrc + "/Cargo.lock";
          outputHashes = {
            "tree-house-0.3.0" = "sha256-sd9JUxcVaAyuI4DG/6qL95h+hC7Sk7BFEspxMKVRRKk=";
          };
        };

        nativeBuildInputs = [
          pkgs.clang
          pkgs.mold
        ];

        doCheck = false;
      };

      rustDevPackages = [
        rustToolchain
        pkgs.rust-analyzer
        pkgs.cargo-watch
        pkgs.cargo-edit
        pkgs.clang
        pkgs.mold
      ];
    in
    {
      treefmt = {
        projectRootFile = "flake.nix";

        programs.rustfmt.enable = true;
        programs.nixfmt.enable = true;

        settings.formatter.cargo-sort = {
          command = "${cargoSortWrapper}/bin/cargo-sort-wrapper";
          options = [ "--workspace" ];
          includes = [
            "Cargo.toml"
            "**/Cargo.toml"
          ];
        };
      };

      packages = {
        rust = rustPackage;
        default = rustPackage;
      };

      checks = {
        rust = rustPackage;
        build = rustPackage;
      };

      devShells = {
        rust = pkgs.mkShell {
          packages = rustDevPackages;
        };

        default = pkgs.mkShell {
          packages = rustDevPackages ++ [ config.treefmt.build.wrapper ];
        };
      };
    };
}
