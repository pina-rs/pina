{ pkgs, lib, config, inputs, ... }:

{
  # https://devenv.sh/packages/
  packages =
    [
      pkgs.binaryen
      pkgs.cargo-binstall
      pkgs.cargo-run-bin
      pkgs.coreutils
      pkgs.curl
      pkgs.dprint
      pkgs.git
      pkgs.jq
      pkgs.libiconv
      pkgs.nixfmt-rfc-style
      pkgs.openssl
      pkgs.protobuf # needed for `solana-test-validator` in tests
      pkgs.rustup
      pkgs.shfmt
    ]
    ++ lib.optionals pkgs.stdenv.isDarwin (
      with pkgs.darwin.apple_sdk;
      [
        frameworks.CoreFoundation
        frameworks.Security
        frameworks.System
        frameworks.SystemConfiguration
      ]
    );

  # https://devenv.sh/tasks/
  # tasks = {
  #   "myproj:setup".exec = "mytool build";
  #   "devenv:enterShell".after = [ "myproj:setup" ];
  # };

  # https://devenv.sh/tests/
  enterTest = ''
    echo "Running tests"
    git --version | grep --color=auto "${pkgs.git.version}"
  '';

  scripts."update:deps" = {
    exec = ''
			set -e
			cargo update
			devenv update
    '';
    description = "Update dependencies.";
  };

  # See full reference at https://devenv.sh/reference/options/
}
