{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      crane,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        craneLib = crane.mkLib pkgs;
        gdguard = craneLib.buildPackage {
          src = ./.;

          cargoExtraArgs = "--bin=gdguard";
        };
      in
      {
        packages.default = gdguard;

        devShells.default = craneLib.devShell {
          inputsFrom = [ gdguard ];
        };

        checks = {
          inherit gdguard;
        };
      }
    );
}
