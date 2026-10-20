{
  description = "linear-cli: work with Linear issues from the command line";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          inherit (pkgs) lib;
          linear = pkgs.rustPlatform.buildRustPackage {
            pname = "linear";
            inherit version;
            src = lib.cleanSource ./.;
            cargoLock.lockFile = ./Cargo.lock;
            cargoBuildFlags = [ "--package" "linear" ];
            doCheck = false;
            meta = {
              description = "Work with Linear issues, projects and teams from the command line";
              homepage = "https://github.com/schpet/linear-cli";
              license = lib.licenses.isc;
              mainProgram = "linear";
              platforms = systems;
            };
          };
        in
        {
          default = linear;
          inherit linear;
        }
      );

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.linear}/bin/linear";
        };
      });

      overlays.default = final: prev: {
        linear = self.packages.${prev.system}.linear;
      };
    };
}
