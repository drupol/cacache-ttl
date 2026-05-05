{
  lib,
  rustPlatform,
}:

rustPlatform.buildRustPackage {
  pname = "cacache-ttl";
  version = "0.2.0";

  __structuredAttrs = true;

  src = lib.fileset.toSource {
    root = ../../..;
    fileset = lib.fileset.unions [
      ../../../Cargo.toml
      ../../../Cargo.lock
      ../../../src
    ];
  };

  cargoHash = "sha256-G4YKF9cysW3Whr1bnAsSvqLiQEhp0kb8mZdNmrI0muw=";
}
