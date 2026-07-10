{
  description = "alpha_bar";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

  outputs =
    { self, nixpkgs, ... }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forEachSystem = nixpkgs.lib.genAttrs supportedSystems;
    in
    {
      devShells = forEachSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo
              clippy
              pkg-config
              rustc
              rustfmt
              llvmPackages.libclang
            ];

            buildInputs = with pkgs; [
              cairo
              libxkbcommon
              pango
              pipewire
              wayland
            ];

            env = {
              LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
              BINDGEN_EXTRA_CLANG_ARGS = "-isystem ${pkgs.glibc.dev}/include";
            };
          };
        }
      );

      packages = forEachSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "alpha_bar";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = with pkgs; [
              llvmPackages.libclang
              pkg-config
            ];
            buildInputs = with pkgs; [
              cairo
              libxkbcommon
              pango
              pipewire
              wayland
            ];
            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
            BINDGEN_EXTRA_CLANG_ARGS = "-isystem ${pkgs.glibc.dev}/include";
            meta.mainProgram = "alpha_bar";
          };
        }
      );

      apps = forEachSystem (system: {
        default = {
          type = "app";
          program = "${nixpkgs.legacyPackages.${system}.lib.getExe self.packages.${system}.default}";
        };
      });

      formatter = forEachSystem (system: nixpkgs.legacyPackages.${system}.nixfmt-tree);
    };
}
