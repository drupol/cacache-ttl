{
  lib,
  cargo,
  clippy,
  cacache-ttl,
}:

cacache-ttl.overrideAttrs (oldAttrs: {
  nativeCheckInputs = (oldAttrs.nativeCheckInputs or [ ]) ++ [
    cargo
    clippy
  ];

  checkPhase = ''
    RUSTFLAGS="-Dwarnings" ${lib.getExe cargo} clippy
  '';
})
