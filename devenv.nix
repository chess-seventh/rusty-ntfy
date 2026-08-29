{
  pkgs,
  lib,
  config,
  inputs,
  ...
}:
let
  # L235 - THE FLEET GATE, as four ordinary hooks of this repository's own
  # (D-115).
  #
  # WHY THIS REPOSITORY NEEDS THEM. `hooks_everywhere.nix` installed the fleet
  # gate on a global `core.hooksPath`, and prek REFUSES to install a
  # repository's own hooks while one points outside the repository - it says so
  # and names the fix. This repository was one of EIGHT that lost its gate to
  # that: `devenv:git-hooks:install` failed, `enterShell` failed with it and
  # `devenv test` returned 1, while `devenv shell -- cmd` still returned 0. Loud
  # in the gate and silent interactively, which is why it went unnoticed.
  #
  # The global value is being removed rather than worked around per repository
  # (Franci, 2026-08-26), and these entries are what replaces it: the gate is
  # reached from inside this repository's own prek run instead of from a config
  # value git obeys. Being one of prek's OWN hooks is the shape that stops
  # fighting a provisioning manager that re-installs on entry - three earlier
  # shapes moved `core.hooksPath` or wrapped prek's shims and each was killed by
  # a reproduction.
  #
  # Built through writeShellApplication so the script is shellchecked at build
  # time and the entries name a store path rather than a working-tree file.
  fleetGateHook = "${
    pkgs.writeShellApplication {
      name = "fleet-gate-hook";
      runtimeInputs = [
        pkgs.coreutils
        pkgs.git
      ];
      text = builtins.readFile ./hooks/fleet-gate-hook;
    }
  }/bin/fleet-gate-hook";
in
{
  dotenv.enable = true;

  env.GREET = "Welcome to the Rusty NTFY";

  packages = with pkgs; [
    git
    jq
    curl
    gnused
    zlib
    cargo-nextest
    cargo-shear
    cargo-llvm-cov
    rustup
  ];

  languages = {
    nix.enable = true;

    rust = {
      enable = true;
      channel = "stable";
      components = [
        "rustc"
        "cargo"
        "clippy"
        "rustfmt"
        "rust-analyzer"
        "rust-std"
        "llvm-tools-preview"
      ];
    };

    shell.enable = true;
  };

  processes = {
    cargo-watch.exec = "cargo-watch";
  };

  tasks = {
    "bash:source_env" = {
      exec = "source $PWD/.env";
      after = [ "devenv:enterShell" ];
    };
  };

  git-hooks.hooks = {
    # L235 - the fleet gate, reached as four of this repo's own hooks. The
    # rationale is in the `let` block at the top of this file; what matters here
    # is that these four are ordinary entries with nothing special about them.
    fleet-gate = {
      enable = true;
      name = "fleet gate";
      stages = [ "pre-commit" ];
      entry = "${fleetGateHook} pre-commit";
      language = "system";
      pass_filenames = false;
      always_run = true;
    };

    # pass_filenames, because git hands commit-msg the message file and the
    # fleet gate's gitlint and commitizen read it. Getting this wrong lints the
    # wrong thing while still exiting 0.
    fleet-gate-commit-msg = {
      enable = true;
      name = "fleet gate (message)";
      stages = [ "commit-msg" ];
      entry = "${fleetGateHook} commit-msg";
      language = "system";
      pass_filenames = true;
      always_run = true;
    };

    fleet-gate-pre-push = {
      enable = true;
      name = "fleet gate (push)";
      stages = [ "pre-push" ];
      entry = "${fleetGateHook} pre-push";
      language = "system";
      pass_filenames = false;
      always_run = true;
    };

    # The commit diary is hooks_everywhere.nix's post-commit hook - NOT
    # pkgs/git-commit-gate, which ships pre-commit and commit-msg only. On a box
    # with no fleet gate there is no diary, and the entry says so per commit.
    fleet-gate-post-commit = {
      enable = true;
      name = "fleet gate (diary)";
      stages = [ "post-commit" ];
      entry = "${fleetGateHook} post-commit";
      language = "system";
      pass_filenames = false;
      always_run = true;
    };

    rusty-commit-saver = {
      enable = true;
      name = "🦀 Rusty Commit Saver";
      stages = [ "post-commit" ];
      after = [
        "commitizen"
        "gitlint"
      ];
      entry = "${
        inputs.rusty-commit-saver.packages.${pkgs.stdenv.hostPlatform.system}.default
      }/bin/rusty-commit-saver";
      pass_filenames = false;
      language = "system";
      always_run = true;
    };

    check-merge-conflicts = {
      name = "🔒 Check Merge Conflicts";
      enable = true;
      stages = [ "pre-commit" ];
    };

    detect-aws-credentials = {
      name = "💭 Detect AWS Credentials";
      enable = true;
      stages = [ "pre-commit" ];
    };

    detect-private-keys = {
      name = "🔑 Detect Private Keys";
      enable = true;
      stages = [ "pre-commit" ];
    };

    end-of-file-fixer = {
      name = "🔚 End of File Fixer";
      enable = true;
      stages = [ "pre-commit" ];
    };

    mixed-line-endings = {
      name = "🔀 Mixed Line Endings";
      enable = true;
      stages = [ "pre-commit" ];
    };

    trim-trailing-whitespace = {
      name = "✨ Trim Trailing Whitespace";
      enable = true;
      stages = [ "pre-commit" ];
    };

    mdsh = {
      enable = true;
      name = "✨ MDSH";
      stages = [ "pre-commit" ];
    };

    treefmt = {
      name = "🌲 TreeFMT";
      enable = true;
      settings.formatters = [
        pkgs.nixfmt
        pkgs.deadnix
        pkgs.yamlfmt
        pkgs.rustfmt
        pkgs.toml-sort
      ];
      stages = [ "pre-commit" ];
    };

    clippy = {
      name = "✂️ Clippy";
      enable = true;
      settings.allFeatures = true;
      extraPackages = [ pkgs.openssl ];
      stages = [ "pre-commit" ];
    };

    commitizen = {
      name = "✨ Commitizen";
      enable = true;
      stages = [ "post-commit" ];
    };

    gitlint = {
      name = "✨ GitLint";
      enable = true;
    };

    markdownlint = {
      name = "✨ MarkdownLint";
      enable = true;
      stages = [ "pre-commit" ];
      settings.configuration = {
        MD033 = false;
        MD013 = {
          line_length = 120;
          tables = false;
        };
        MD041 = false;
      };
    };

  };

  scripts = {
    cclippy = {
      description = ''
        Run clippy
      '';
      exec = ''
        cargo clippy --all-targets -- -W clippy::pedantic -A clippy::missing_errors_doc -A clippy::must_use_candidate -A clippy::module_name_repetitions -A clippy::doc_markdown -A clippy::missing_panics_doc
      '';
    };

    pre-check = {
      description = ''
        runs linters, tests, and builds to prepare commit/push (more extensively than pre-commit hook)
      '';
      exec = ''
        #!/usr/bin/env bash
        set -euo pipefail

        if [ -f .env.testing ]; then
            source .env.testing
        fi

        treefmt
        cargo clippy --all-targets -- -D warnings
        cargo shear --fix
        cargo llvm-cov --html nextest --no-fail-fast
      '';
    };
  };

  enterShell = ''
    echo "Sourcing .env with evaluated command substitution…"
    if [ -f ".env" ]; then
      eval "$(<.env)"
    fi

    if [ -f ".env-private" ]; then
      eval "$(<.env-private)"
    fi

    echo
    echo 💡 Helper scripts to ease development process:
    echo
    ${pkgs.gnused}/bin/sed -e 's| |••|g' -e 's|=| |' <<EOF | ${pkgs.util-linuxMinimal}/bin/column -t | ${pkgs.gnused}/bin/sed -e 's|^|• |' -e 's|••| |g'
    ${lib.generators.toKeyValue { } (lib.mapAttrs (name: value: value.description) config.scripts)}
    EOF
    echo
  '';
}
