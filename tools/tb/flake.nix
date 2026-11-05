# `tb`, built against the shared toolkit. Locked by the repo root flake.lock.
{
    inputs = {
        nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

        term-toolkit = {
            url = "path:../term-toolkit";
            inputs.nixpkgs.follows = "nixpkgs";
        };
    };

    outputs =
        {
            self,
            nixpkgs,
            term-toolkit,
            ...
        }:
        {
            packages = term-toolkit.lib.forEachSystem (sys: {
                default = term-toolkit.lib.mkTool {
                    pkgs = nixpkgs.legacyPackages.${sys};
                    name = "tb";
                    src = self;
                };
            });
        };
}
