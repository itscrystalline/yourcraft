{ pkgs, lib, config, inputs, ... }:

{

  cachix.enable = false;

  # https://devenv.sh/packages/
  packages = [ pkgs.git ];

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

  # https://devenv.sh/processes/
  # processes.cargo-watch.exec = "cargo-watch";

  # https://devenv.sh/service
}
