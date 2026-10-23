{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      ...
    }:
    let
      inherit (nixpkgs) lib;
      eachSystem = lib.genAttrs lib.systems.flakeExposed;
    in
    {
      devShells = eachSystem (
        system:
        let
          overlays = [ (import rust-overlay) ];
          pkgs = import nixpkgs {
            inherit system overlays;
          };
        in
        {
          default = pkgs.mkShell {
            buildInputs = with pkgs; [
              cargo-nextest
              cargo-edit
              cargo-expand
              cargo-bloat
              just
              (rust-bin.fromRustupToolchainFile ./rust-toolchain.toml)
            ];
            # The C bindings build unoptimized in dev; fortify only warns there.
            hardeningDisable = [ "fortify" ];
            # For size/measure.sh: llvm-size reads PE and ELF for every arch, and
            # an unwrapped clang builds C for the COFF/ELF cross targets (the
            # wrapped one adds host hardening flags that break them).
            LLVM_SIZE = lib.getExe' pkgs.llvmPackages.llvm "llvm-size";
            HASHSIZE_CC = lib.getExe' pkgs.llvmPackages.clang-unwrapped "clang";
          };
        }
      );
    };
}
