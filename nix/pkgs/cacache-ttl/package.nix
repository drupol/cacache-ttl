{
  lib,
  rustPlatform,
}:

rustPlatform.buildRustPackage {
  pname = "cacache-ttl";
  version = "0.2.1";

  __structuredAttrs = true;

  src = lib.fileset.toSource {
    root = ../../..;
    fileset = lib.fileset.unions [
      ../../../Cargo.toml
      ../../../Cargo.lock
      ../../../src
    ];
  };

  cargoHash = "sha256-dGj8vAqgdnEUPmc/jZrNvAoFS+o6lwTjDC/k3w/NakU=";
}
