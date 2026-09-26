{
  description = "Nix flake for linear-cli";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    deno2nix = {
      url = "github:aMOPel/deno2nix?ref=custom-made-fetcher";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, deno2nix }:
    let
      lib = nixpkgs.lib;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = lib.genAttrs systems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          deno2nixLib = (import deno2nix { inherit pkgs; }).lib;
          version = (builtins.fromJSON (builtins.readFile ./deno.json)).version;
          linear = deno2nixLib.buildDenoPackage {
            pname = "linear";
            inherit version;
            src = lib.cleanSource ./.;
            denoDepsHash = "sha256-jGqice4hH4RW2o7Q4VhwUm8G/EUb98AdJ/Z1jrXMeGE=";

            buildPhase = ''
              runHook preBuild
              deno run --no-check --cached-only --allow-all npm:@graphql-codegen/cli/graphql-codegen-esm
              runHook postBuild
            '';

            installPhase = ''
              runHook preInstall
              mkdir -p $out/share/linear $out/bin $out/share/doc/linear $out/share/licenses/linear
              cp -r src graphql deno.json deno.lock $out/share/linear/
              cp -r vendor .deno node_modules $out/share/linear/
              cp README.md CHANGELOG.md $out/share/doc/linear/
              cp LICENSE $out/share/licenses/linear/
              cat > $out/bin/linear <<EOF
#!${pkgs.runtimeShell}
export DENO_DIR=$out/share/linear/.deno
cd $out/share/linear
exec ${pkgs.deno}/bin/deno run --no-check --cached-only --allow-all src/main.ts "\$@"
EOF
              chmod +x $out/bin/linear
              runHook postInstall
            '';

            meta = {
              description = "CLI for Linear.app";
              homepage = "https://github.com/schpet/linear-cli";
              license = lib.licenses.mit;
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
