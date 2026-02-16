# Shell dependencies for terminal IPC tests.
# Used by NIX_TESTS=1 to resolve shell binaries via nix-shell.
{
  pkgs ? import <nixpkgs> { },
}:

pkgs.mkShell {
  packages = [
    pkgs.bash
    pkgs.zsh
    pkgs.nushell
    pkgs.fish
    pkgs.netcat
  ];
}
