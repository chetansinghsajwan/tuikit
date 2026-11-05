# Our Rust code: the term-toolkit libraries, the tools built on them, and the helix fork.
# Locking happens in the repo root flake.lock; this flake has no lock of its own.
#
# All tools share one Cargo workspace (Cargo.toml, Cargo.lock), so crane compiles the
# dependencies once (`cargoArtifacts`) and each tool only compiles our own code on top.
# The helix fork keeps its own workspace and is built from the same flake.
{
    inputs = {
        nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

        llib = {
            url = "path:../lib";
            inputs.nixpkgs.follows = "nixpkgs";
        };

        # Splits cargo builds into a cached dependency build and our code.
        crane.url = "github:ipetkov/crane";
    };

    outputs =
        {
            nixpkgs,
            llib,
            crane,
            ...
        }:
        let
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

            workspaceDirs = [
                "term-toolkit"
                "pick"
                "pv"
                "files"
                "gu"
                "tb"
                "nu"
                "mdexport"
            ];
            helixDirs = [
                "helix"
                "term-toolkit"
            ];

            # The part of this directory a build needs, so editing anything else (nix files, docs,
            # the other workspace) does not rebuild it. Never local build output (target/) or the
            # fetched helix grammar sources.
            sourceOf =
                lib: dirs: files:
                lib.cleanSourceWith {
                    src = ./.;
                    filter =
                        path: _:
                        let
                            rel = lib.removePrefix "${toString ./.}/" (toString path);
                            inDir = d: rel == d || lib.hasPrefix "${d}/" rel;
                        in
                        (rel == (toString ./.) || builtins.elem rel files || lib.any inDir dirs)
                        && baseNameOf path != "target"
                        && !(lib.hasSuffix "runtime/grammars/sources" rel);
                };
        in
        {
            packages = llib.forEachSystem (
                sys:
                let
                    pkgs = nixpkgs.legacyPackages.${sys};
                    craneLib = crane.mkLib pkgs;

                    common = {
                        pname = "pkgs";
                        version = "0.1.0";
                        src = sourceOf pkgs.lib workspaceDirs [
                            "Cargo.toml"
                            "Cargo.lock"
                        ];
                        strictDeps = true;
                    };

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
                (builtins.mapAttrs mkTool tools)
                // {
                    helix = pkgs.callPackage ./helix/default.nix {
                        inherit craneLib;
                        src = sourceOf pkgs.lib helixDirs [ ];
                    };
                }
            );

            # `nix flake check`: the workspace's own tests.
            checks = llib.forEachSystem (
                sys:
                let
                    pkgs = nixpkgs.legacyPackages.${sys};
                    craneLib = crane.mkLib pkgs;
                    common = {
                        pname = "pkgs";
                        version = "0.1.0";
                        src = sourceOf pkgs.lib workspaceDirs [
                            "Cargo.toml"
                            "Cargo.lock"
                        ];
                        strictDeps = true;
                        nativeCheckInputs = [ pkgs.git ];
                        cargoExtraArgs = "--locked --workspace";
                    };
                in
                {
                    tests = craneLib.cargoTest (common // { cargoArtifacts = craneLib.buildDepsOnly common; });
                }
            );

            devShells = llib.forEachSystem (
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
