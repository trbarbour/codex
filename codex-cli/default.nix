{ pkgs, monorepo-deps ? [], ... }:
let
  nodejs = if pkgs ? nodejs_22 then pkgs.nodejs_22 else pkgs.nodejs;
  pnpmHome =
    let
      home = builtins.getEnv "HOME";
    in
    if home == "" then throw "HOME environment variable not set" else "${home}/.pnpm";
  env = {
    PNPM_HOME = pnpmHome;
  };
  commonPackages = monorepo-deps ++ [
    nodejs
  ];
in
rec {
  package = pkgs.stdenvNoCC.mkDerivation {
    pname = "codex-cli";
    version = "0.0.0-dev";
    src = ./.;
    dontConfigure = true;
    dontBuild = true;
    installPhase = ''
      runHook preInstall
      mkdir -p "$out"
      cp -r . "$out/"
      runHook postInstall
    '';
    meta = with pkgs.lib; {
      description = "OpenAI Codex command-line interface wrapper";
      homepage = "https://github.com/openai/codex";
      license = licenses.asl20;
    };
  };

  devShell = pkgs.mkShell {
    inherit env;
    name = "codex-cli-dev";
    packages = commonPackages;
    shellHook = ''
      echo "Entering development shell for codex-cli"
      mkdir -p "$PNPM_HOME"
      export PATH="$PNPM_HOME:$PATH"
    '';
  };

  app = {
    type = "app";
    program = "${pkgs.writeShellScriptBin "codex" ''
      exec ${nodejs}/bin/node ${package}/bin/codex.js "$@"
    ''}/bin/codex";
  };
}
