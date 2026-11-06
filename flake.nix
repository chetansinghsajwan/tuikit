# tuikit: the toolkit libraries (crates/) and the tools built on them (tools/).
#
# All tools share one Cargo workspace (Cargo.toml, Cargo.lock), so crane compiles the
# dependencies once (`cargoArtifacts`) and each tool only compiles our own code on top.
{
    inputs = {
        nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

        # Splits cargo builds into a cached dependency build and our code.
        crane.url = "github:ipetkov/crane";
    };

    outputs =
        { nixpkgs, crane, ... }:
        let
            forEachSystem = nixpkgs.lib.genAttrs [
                "x86_64-linux"
                "aarch64-linux"
                "x86_64-darwin"
                "aarch64-darwin"
            ];

            # Cargo package of each tool; the binary has the short name.
            tools = {
                pick = "pick-cli";
                pv = "pv-cli";
                files = "files-cli";
                gu = "gu";
                tb = "tb-cli";
                nu = "nu";
                mdexport = "mdexport";
            };

            # The part of this repo a build needs, so editing anything else (nix files, docs)
            # does not rebuild it. Never local build output (target/).
            sourceOf =
                lib:
                lib.cleanSourceWith {
                    src = ./.;
                    filter =
                        path: _:
                        let
                            rel = lib.removePrefix "${toString ./.}/" (toString path);
                            inDir = d: rel == d || lib.hasPrefix "${d}/" rel;
                        in
                        (
                            rel == (toString ./.)
                            || builtins.elem rel [
                                "Cargo.toml"
                                "Cargo.lock"
                            ]
                            || lib.any inDir [
                                "crates"
                                "tools"
                            ]
                        )
                        && baseNameOf path != "target";
                };

            commonFor = pkgs: {
                pname = "tuikit";
                version = "0.1.0";
                src = sourceOf pkgs.lib;
                strictDeps = true;
            };
        in
        {
            packages = forEachSystem (
                sys:
                let
                    pkgs = nixpkgs.legacyPackages.${sys};
                    craneLib = crane.mkLib pkgs;
                    common = commonFor pkgs;

                    # Every dependency of the whole workspace, built once from the manifests alone:
                    # editing our code does not rebuild them.
                    cargoArtifacts = craneLib.buildDepsOnly (common // { cargoExtraArgs = "--locked --workspace"; });

                    # Tests run in the dev loop and in `checks`, not here.
                    mkTool =
                        name: crate:
                        craneLib.buildPackage (
                            common
                            // {
                                inherit cargoArtifacts;
                                pname = name;
                                cargoExtraArgs = "--locked -p ${crate}";
                                doCheck = false;
                            }
                        );
                in
                builtins.mapAttrs mkTool tools
            );

            # `nix flake check`: the workspace's own tests.
            checks = forEachSystem (
                sys:
                let
                    pkgs = nixpkgs.legacyPackages.${sys};
                    craneLib = crane.mkLib pkgs;
                    common = (commonFor pkgs) // {
                        nativeCheckInputs = [ pkgs.git ];
                        cargoExtraArgs = "--locked --workspace";
                    };
                in
                {
                    tests = craneLib.cargoTest (common // { cargoArtifacts = craneLib.buildDepsOnly common; });
                }
            );

            devShells = forEachSystem (
                sys:
                let
                    pkgs = nixpkgs.legacyPackages.${sys};
                in
                {
                    default = pkgs.mkShell {
                        packages = with pkgs; [
                            cargo
                            rustc
                            clippy
                            rustfmt
                            rust-analyzer
                            gcc
                            git
                        ];
                    };
                }
            );
        };
}
