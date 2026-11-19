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

  outputs = {
    nixpkgs,
    crane,
    ...
  }: let
    forEachSystem = nixpkgs.lib.genAttrs [
      "x86_64-linux"
      "aarch64-linux"
      "x86_64-darwin"
      "aarch64-darwin"
    ];

    # Cargo package of each tool; the binary has the short name.
    tools = {
      pick = "tuikit-pick-cli";
      pv = "tuikit-pv-cli";
      files = "tuikit-files-cli";
      gu = "tuikit-gu";
      kanata-status = "tuikit-kanata-status";
      tb = "tuikit-tb-cli";
      nu = "tuikit-nu";
      mdexport = "tuikit-mdexport";
    };

    # The part of this repo a build needs, so editing anything else (nix files, docs)
    # does not rebuild it. Never local build output (target/).
    sourceOf = lib:
      lib.cleanSourceWith {
        src = ./.;
        filter = path: _: let
          rel = lib.removePrefix "${toString ./.}/" (toString path);
          inDir = d: rel == d || lib.hasPrefix "${d}/" rel;
        in
          (
            rel
            == (toString ./.)
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

    # Every tool built against the given package set.
    toolsFor = pkgs: let
      craneLib = crane.mkLib pkgs;
      common = commonFor pkgs;

      # Every dependency of the whole workspace, built once from the manifests alone:
      # editing our code does not rebuild them.
      cargoArtifacts = craneLib.buildDepsOnly (common // {cargoExtraArgs = "--locked --workspace";});

      # Tests run in the dev loop and in `checks`, not here.
      mkTool = name: crate:
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
      builtins.mapAttrs mkTool tools;
  in {
    # On Linux every tool also has a `<tool>-static` build against musl: one self-contained
    # binary with no `/nix/store` references, which is what the GitHub Release ships.
    packages = forEachSystem (
      sys: let
        pkgs = nixpkgs.legacyPackages.${sys};
        inherit (nixpkgs) lib;
      in
        toolsFor pkgs
        // lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux (
          lib.mapAttrs' (name: lib.nameValuePair "${name}-static") (toolsFor pkgs.pkgsStatic)
        )
    );

    # `nix flake check`: the workspace's own tests.
    checks = forEachSystem (
      sys: let
        pkgs = nixpkgs.legacyPackages.${sys};
        craneLib = crane.mkLib pkgs;
        common =
          (commonFor pkgs)
          // {
            nativeCheckInputs = [pkgs.git];
            cargoExtraArgs = "--locked --workspace";
          };
      in {
        tests = craneLib.cargoTest (common // {cargoArtifacts = craneLib.buildDepsOnly common;});
      }
    );

    devShells = forEachSystem (
      sys: let
        pkgs = nixpkgs.legacyPackages.${sys};
      in {
        default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            rustc
            clippy
            rustfmt
            rust-analyzer
            gcc
            git

            # Task runner, hooks and formatter runner.
            go-task
            lefthook
            treefmt
            cocogitto

            # Formatters and linters.
            alejandra
            statix
            deadnix
            taplo
            dprint
            rumdl
            yamlfmt
            actionlint
            zizmor
            typos

            # Tests and vulnerability scanners.
            cargo-nextest
            cargo-deny
            osv-scanner
            gitleaks
            semgrep
          ];
        };
      }
    );
  };
}
