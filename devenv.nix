{ pkgs, lib, config, inputs, ... }:

{
  cachix.enable = false;

  env.LD_LIBRARY_PATH = "${pkgs.libglvnd}/lib";

  # https://devenv.sh/packages/
  packages = [ pkgs.git pkgs.xorg.libX11 ];

  # https://devenv.sh/languages/
  languages.rust = {
    enable = true;
    channel = "stable";
    components = [ "rustc" "cargo" "clippy" "rustfmt" "rust-analyzer" "rust-src" ];
  };
  languages.python = {
    enable = true;
    package = pkgs.python312Full;
    venv.enable = true;
    venv.requirements = builtins.readFile ./requirements.txt;
  };
}
