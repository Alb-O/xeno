# Language server dependencies for LSP integration tests.
# Used by LSP_TESTS=1 to provide language servers via nix-shell.
{
  pkgs ? import <nixpkgs> { },
}:

pkgs.mkShell {
  packages = [
    pkgs.rust-analyzer  # Rust LSP - primary test target
    pkgs.rustc          # Rust compiler (needed by rust-analyzer)
    pkgs.cargo          # Cargo (needed by rust-analyzer)
  ];
}
