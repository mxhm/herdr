{
  description = "herdr deploy flake — pulls the fork's patched package";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    herdr.url = "path:..";
  };

  outputs =
    { self, nixpkgs, herdr }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (system: {
        default = herdr.packages.${system}.default;
      });
      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${herdr.packages.${system}.default}/bin/herdr";
          meta.description = "Run herdr (patched)";
        };
      });
    };
}
