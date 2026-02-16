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
      rustToolchain = rustPkgs.rust-bin.fromRustupToolchainFile (rootSrc + "/rust_toolchain.toml");
      rustPlatform = pkgs.makeRustPlatform {
        cargo = rustToolchain;
        rustc = rustToolchain;
      };

      guiRuntimeDeps = with pkgs; [
        pkg-config
        gtk3
        libGL
        libxkbcommon
        openssl
        wayland
        vulkan-loader
        libx11
        libxcursor
        libxext
        libxi
        libxinerama
        libxrandr
        libxcb
      ];

      guiLibraryPath = pkgs.lib.makeLibraryPath guiRuntimeDeps;

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
            # required for iced upstream
            "cryoglyph-0.1.0" = "sha256-Rnu2Du7zi/FCFgli1pPYIRNypuxBxcrOyIZzyWGezPE=";
            "dpi-0.1.1" = "sha256-pQn1lCFSJMkjUfHoggEzMHnm5k+Chnzi5JEDjahnjUA=";
            "iced-0.15.0-dev" = "sha256-VXdzZDDUjbPApvfEW1uqW9pVc65giJRmju9YN2IotHM=";
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

        programs.nixfmt.enable = true;

        programs.rustfmt.enable = true;
        programs.rustfmt.package = rustToolchain;

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
          packages = rustDevPackages ++ guiRuntimeDeps;
          LD_LIBRARY_PATH = guiLibraryPath;
        };

        default = pkgs.mkShell {
          packages = rustDevPackages ++ guiRuntimeDeps ++ [ config.treefmt.build.wrapper ];
          LD_LIBRARY_PATH = guiLibraryPath;
        };
      };
    };
}
