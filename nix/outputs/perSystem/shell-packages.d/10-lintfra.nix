# Lintfra packages for devshell
{ pkgs, self', ... }:
[
  pkgs.ast-grep
  pkgs.yq-go
  pkgs.jq
  self'.packages.lint
]
