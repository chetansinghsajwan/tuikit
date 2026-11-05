# The shared terminal toolkit: Rust library crates only (design tokens, git, preview, picker,
# panels). The tools and the helix fork are flakes of their own that build against this source
# and use `lib.mkTool` from here.
#
# Locking happens in the repo root flake.lock; this flake has no lock of its own.
{
    inputs = {
        nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

        llib = {
            url = "path:../../lib";
            inputs.nixpkgs.follows = "nixpkgs";
        };

        # Splits cargo builds into a cached dependency build and our code.
        crane.url = "github:ipetkov/crane";
    };

    outputs =
        {
            self,
            nixpkgs,
            llib,
            crane,
            ...
        }:
        let
            # No local build output (target/, fetched grammar sources), so a build never depends on it.
            clean =
                pkgs: src:
                pkgs.lib.cleanSourceWith {
                    inherit src;
                    filter = path: _: baseNameOf path != "target" && !(pkgs.lib.hasSuffix "/runtime/grammars/sources" path);
                };

            # Source tree `<name>/` (the tool) beside `term-toolkit/`, so the tool's
            # `../term-toolkit/<crate>` path dependencies resolve inside the build directory.
            composeSrc =
                {
                    pkgs,
                    name,
                    src,
                }:
                pkgs.runCommand "${name}-src" { } ''
                    mkdir -p $out
                    cp -r --no-preserve=mode ${clean pkgs src} $out/${name}
                    cp -r --no-preserve=mode ${clean pkgs self} $out/term-toolkit
                '';

            # One Cargo package (the tool's `src`, with its own Cargo.lock) built with crane:
            # a cached dependency build, then our code. Installs every binary of the package.
            # Tests run in the dev loop (`cargo test`), not here.
            mkTool =
                {
                    pkgs,
                    name,
                    src,
                    version ? "0.1.0",
                    cargoExtraArgs ? "--locked",
                    extraArgs ? { },
                }:
                let
                    craneLib = crane.mkLib pkgs;
                    commonArgs = {
                        pname = name;
                        inherit version cargoExtraArgs;
                        src = composeSrc { inherit pkgs name src; };
                        # The package is in a subdirectory, next to `term-toolkit`.
                        postUnpack = "cd $sourceRoot/${name}; sourceRoot=.";
                        cargoLock = src + "/Cargo.lock";
                        strictDeps = true;
                        doCheck = false;
                    }
                    // extraArgs;
                    cargoArtifacts = craneLib.buildDepsOnly commonArgs;
                in
                craneLib.buildPackage (commonArgs // { inherit cargoArtifacts; });
        in
        {
            lib = {
                inherit (llib) forEachSystem;
                inherit mkTool composeSrc;
                craneFor = crane.mkLib;
            };

            # `nix flake check`: the toolkit's own tests.
            checks = llib.forEachSystem (
                sys:
                let
                    pkgs = nixpkgs.legacyPackages.${sys};
                    craneLib = crane.mkLib pkgs;
                    args = {
                        pname = "term-toolkit";
                        version = "0.1.0";
                        src = clean pkgs self;
                        strictDeps = true;
                        nativeCheckInputs = [ pkgs.git ];
                    };
                in
                {
                    tests = craneLib.cargoTest (
                        args // { cargoArtifacts = craneLib.buildDepsOnly args; }
                    );
                }
            );
        };
}
