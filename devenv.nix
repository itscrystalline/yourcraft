{ pkgs, lib, config, inputs, ... }:

let 
  cc_aarch64 = pkgs.pkgsCross.aarch64-multiplatform.stdenv.cc; 
in {
  cachix.enable = false;

  env.LD_LIBRARY_PATH = "${pkgs.libglvnd}/lib";

  # https://devenv.sh/packages/
  packages = [ 
    pkgs.git 
    pkgs.xorg.libX11 
    pkgs.libz 
    pkgs.SDL2
    pkgs.evcxr
  ] ++ (with pkgs.python312Packages; [
    (pygame.overrideAttrs (oldAttrs: newAttrs: {
        env.PYGAME_DETECT_AVX2 = 1;
    }))
  ]) ++ pkgs.lib.optional (pkgs.system == "x86_64-linux") pkgs.gcc_multi;

  env.CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER = "${cc_aarch64}/bin/${cc_aarch64.targetPrefix}cc";
  env.CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER = "${cc_aarch64}/bin/${cc_aarch64.targetPrefix}cc";
  # https://devenv.sh/languages/
  languages.rust = {
    enable = true;
    channel = "stable";
    components = [ "rustc" "cargo" "clippy" "rustfmt" "rust-analyzer" "rust-src" ];
    targets = [ "x86_64-unknown-linux-gnu" "aarch64-unknown-linux-gnu" "i686-unknown-linux-gnu" "x86_64-unknown-linux-musl" "aarch64-unknown-linux-musl" ];
  };
  languages.python = {
    enable = true;
    package = pkgs.python312Full;
  };
  
  devcontainer.enable = true;
  scripts.repl.exec = "evcxr";
}
