{
  lib,
  rustPlatform,
}:

rustPlatform.buildRustPackage {
  pname = "cacache-ttl";
  version = "0.1.1";

  __structuredAttrs = true;

  src = lib.fileset.toSource {
    root = ../../..;
    fileset = lib.fileset.unions [
      ../../../Cargo.toml
      ../../../Cargo.lock
      ../../../src
    ];
  };

  cargoHash = "sha256-LLFIqT4+4FvqqMSlfwYmlkrUSM7WF/Kg1VFtX+9BDiI=";
}
