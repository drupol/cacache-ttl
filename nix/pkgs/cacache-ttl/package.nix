{
  lib,
  rustPlatform,
}:

rustPlatform.buildRustPackage {
  pname = "cacache-ttl";
  version = "0.2.2";

  __structuredAttrs = true;

  src = lib.fileset.toSource {
    root = ../../..;
    fileset = lib.fileset.unions [
      ../../../Cargo.toml
      ../../../Cargo.lock
      ../../../src
    ];
  };

  cargoHash = "sha256-VGHAhMFFwWLbedSvZ4umNmh8x/qsWUhOwJZApp/OGkE=";
}
