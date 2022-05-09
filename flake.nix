{
  description = "Flake for mdbook-kroki-preprocessor";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        defaultPackage = with pkgs; rustPlatform.buildRustPackage {
          pname = "mdbook-kroki-preprocessor";
          version = "0.1.0";
          nativeBuildInputs = [ pkg-config ];
          buildInputs = [ openssl ];

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          src = ./.;
      };
    });
}
