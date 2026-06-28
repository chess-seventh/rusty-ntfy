{
  lib,
  rustPlatform,
}:

rustPlatform.buildRustPackage {
  pname = "rusty-ntfy";
  version = "2.3.2";

  # Only the cargo inputs — keep target/, .git, devenv out of the build.
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../src
    ];
  };

  cargoLock.lockFile = ../Cargo.lock;

  # No native deps: reqwest uses rustls (not openssl) and the vestigial
  # diesel/postgres dependency was removed, so there is no libpq to link.

  meta = {
    description = "Tailscale mesh prober that emits ntfy alerts (outbound-only)";
    homepage = "https://github.com/chess-seventh/rusty-ntfy";
    license = lib.licenses.mit;
    mainProgram = "rusty-ntfy";
    platforms = lib.platforms.linux;
  };
}
