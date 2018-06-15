with import <nixpkgs> {};
stdenv.mkDerivation {
  name = "lseq";
  buildInputs = [
    bashInteractive
    rustup
  ];
}
