# Reusable NixOS module for rusty-ntfy. `self` is the flake, used only to
# default the package to this flake's build for the host's system.
self:
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.rusty-ntfy;
in
{
  options.services.rusty-ntfy = {
    enable = lib.mkEnableOption "rusty-ntfy Tailscale mesh prober (ntfy alerts, outbound-only)";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.rusty-ntfy;
      defaultText = lib.literalExpression "rusty-ntfy.packages.\${system}.rusty-ntfy";
      description = "The rusty-ntfy package to run.";
    };

    configFile = lib.mkOption {
      type = lib.types.path;
      example = "/run/secrets/rusty-ntfy.ini";
      description = ''
        Path to the INI config holding the ntfy topic — typically a
        sops-nix secret rendered at activation, never hand-placed. Must
        contain an [ntfy-topic] section with a topic_name key; the literal
        token HOSTNAME inside topic_name is replaced per probed peer.
      '';
    };

    socketPath = lib.mkOption {
      type = lib.types.str;
      default = "/run/tailscale/tailscaled.sock";
      description = "Path to the tailscaled LocalAPI unix socket.";
    };

    user = lib.mkOption {
      type = lib.types.str;
      default = "root";
      description = ''
        User to run the prober as. Must be able to read both socketPath and
        configFile; defaults to root because the tailscaled socket is
        root-owned on NixOS.
      '';
    };

    onCalendar = lib.mkOption {
      type = lib.types.str;
      default = "*:0/15";
      example = "*:0/5";
      description = "systemd OnCalendar expression driving the periodic probe.";
    };

    onBootSec = lib.mkOption {
      type = lib.types.str;
      default = "2min";
      description = "Delay after boot before the first probe runs.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.rusty-ntfy = {
      description = "rusty-ntfy Tailscale mesh prober (ntfy alerts)";
      after = [
        "network-online.target"
        "tailscaled.service"
      ];
      wants = [ "network-online.target" ];

      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${lib.getExe cfg.package} ${cfg.socketPath}";
        User = cfg.user;
        Environment = [ "RUSTY_NTFY_CONFIG=${cfg.configFile}" ];

        # Outbound-only, never a message bus: probe the mesh, POST to ntfy,
        # exit. No listening sockets; tight filesystem and syscall sandbox.
        # AF_NETLINK is required — local-ip-address reads interfaces via it.
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        RestrictAddressFamilies = [
          "AF_INET"
          "AF_INET6"
          "AF_UNIX"
          "AF_NETLINK"
        ];
        RestrictNamespaces = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        SystemCallFilter = [ "@system-service" ];
        SystemCallArchitectures = "native";
      };
    };

    systemd.timers.rusty-ntfy = {
      description = "Schedule the rusty-ntfy Tailscale mesh probe";
      wantedBy = [ "timers.target" ];
      timerConfig = {
        OnBootSec = cfg.onBootSec;
        OnCalendar = cfg.onCalendar;
        Persistent = true;
        Unit = "rusty-ntfy.service";
      };
    };
  };
}
