{
  description = "rusty-ntfy — Tailscale mesh prober that emits ntfy alerts (outbound-only)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (pkgs: rec {
        rusty-ntfy = pkgs.callPackage ./nix/package.nix { };
        default = rusty-ntfy;
      });

      nixosModules = rec {
        rusty-ntfy = import ./nix/module.nix self;
        default = rusty-ntfy;
      };

      checks = forAllSystems (
        pkgs:
        let
          system = pkgs.stdenv.hostPlatform.system;
        in
        {
          # Builds + runs the cargo tests via buildRustPackage's check phase.
          package = self.packages.${system}.rusty-ntfy;

          # Proves the NixOS module evaluates: instantiate a throwaway system
          # with the service enabled and force the rendered unit + timer.
          module-eval =
            let
              sys = nixpkgs.lib.nixosSystem {
                inherit system;
                modules = [
                  self.nixosModules.rusty-ntfy
                  {
                    services.rusty-ntfy = {
                      enable = true;
                      configFile = "/run/secrets/rusty-ntfy.ini";
                    };
                  }
                ];
              };
            in
            pkgs.runCommand "rusty-ntfy-module-eval" { } ''
              {
                echo "${sys.config.systemd.services.rusty-ntfy.serviceConfig.ExecStart}"
                echo "${sys.config.systemd.timers.rusty-ntfy.timerConfig.OnCalendar}"
              } > "$out"
            '';
        }
      );

      formatter = forAllSystems (pkgs: pkgs.nixfmt);
    };
}
