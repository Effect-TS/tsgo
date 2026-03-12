{
  description = "Self-contained Effect Language Service (TypeScript-Go)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    nixpkgsUnstable.url = "github:NixOS/nixpkgs/nixos-unstable";
    typescript-go-src = {
      url = "github:microsoft/typescript-go/dfcdea6d6989eab87cad7a6075948845e349ae4c?submodules=1";
      flake = false;
    };
    typescript-src = {
      url = "github:microsoft/TypeScript/2a3bed2b4265fa1173c88771a21ce044e6480f75";
      flake = false;
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      nixpkgsUnstable,
      typescript-src,
      typescript-go-src,
    }:
    let
      lib = nixpkgs.lib;
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      /*
       This hash covers the exported `GOMODCACHE` produced by `go mod download all`.
       It intentionally does not cover generated files or compiled artifacts, so one
       hash can be shared across systems as long as the downloaded module set stays
       the same.

       Update this hash whenever the workspace's downloaded module set changes,
       typically after changes to `go.mod`, `go.work`, or any transitive Go module
       requirements pulled in by this repository.

       Refresh workflow:
       1. Temporarily set this value to `lib.fakeHash`.
       2. Run `nix build .#effect-tsgo --no-write-lock-file`.
       3. Copy the reported `got: sha256-...` value back here.

       This is a good candidate for automation later, for example via a small script
       that swaps in `lib.fakeHash`, runs the build, and updates the value.
      */
      workspaceModuleCacheHash = "sha256-Fz7I/aDvzexCdqrG7Q3B9qpj1CArItWYkr26DV7tIeo=";
      forAllSystems =
        f: lib.genAttrs supportedSystems (system: f system (import nixpkgs { inherit system; }));
    in
    {
      packages = forAllSystems (
        system: pkgs:
        let
          root = toString ./.;
          pkgsUnstable = import nixpkgsUnstable { inherit system; };
          patchEntries = builtins.readDir ./_patches;
          patchFiles = builtins.filter (
            name: patchEntries.${name} == "regular" && lib.hasSuffix ".patch" name
          ) (builtins.attrNames patchEntries);
          sortedPatchFiles = builtins.sort builtins.lessThan patchFiles;
          rootSrc = lib.cleanSourceWith {
            src = ./.;
            filter =
              path: type:
              let
                pathString = toString path;
                relPath = if pathString == root then "" else lib.removePrefix "${root}/" pathString;
                topLevel = if relPath == "" then "" else builtins.head (lib.splitString "/" relPath);
              in
              lib.cleanSourceFilter path type
              && !builtins.elem topLevel [
                ".direnv"
                ".git"
                ".repos"
                "build"
                "built"
                "coverage"
                "node_modules"
                "tmp"
                "typescript-go"
              ];
          };
          patchedTypescriptGo = pkgs.applyPatches {
            name = "patched-typescript-go-source";
            src = typescript-go-src;
            patches = builtins.map (name: ./. + "/_patches/${name}") sortedPatchFiles;
          };
          src = pkgs.runCommandNoCC "effect-tsgo-source" { } ''
            mkdir source
            cp -R ${rootSrc}/. source/
            chmod -R u+w source
            cp -R ${patchedTypescriptGo} source/typescript-go
            chmod -R u+w source/typescript-go
            mkdir -p source/typescript-go/_submodules
            if [ -d source/typescript-go/_submodules/TypeScript ]; then
              rmdir source/typescript-go/_submodules/TypeScript
            fi
            ln -s ${typescript-src} source/typescript-go/_submodules/TypeScript
            cp -R source $out
            chmod -R a-w $out
          '';

          workspaceModuleCache = pkgs.stdenvNoCC.mkDerivation {
            name = "effect-tsgo-workspace-gomodcache";
            inherit src;
            nativeBuildInputs = [ pkgsUnstable.go_1_26 ];
            env = {
              CGO_ENABLED = 0;
              GOWORK = "auto";
            };
            outputHashMode = "recursive";
            outputHash = workspaceModuleCacheHash;
            buildPhase = ''
              runHook preBuild
              export HOME="$TMPDIR"
              export GOPATH="$TMPDIR/go"
              export GOMODCACHE="$GOPATH/pkg/mod"
              export GOCACHE="$TMPDIR/go-cache"
              mkdir -p "$GOMODCACHE"

              cp -R "$src"/. work
              chmod -R u+w work

              (
                cd work
                go mod download all
              )
              runHook postBuild
            '';
            installPhase = ''
              runHook preInstall
              rm -rf "$GOMODCACHE/cache/download/sumdb"
              cp -R "$GOMODCACHE" $out
              runHook postInstall
            '';
            dontFixup = true;
          };

          tsgo = pkgs.stdenvNoCC.mkDerivation {
            pname = "effect-tsgo";
            version = "0.0.0";
            inherit src;
            nativeBuildInputs = [ pkgsUnstable.go_1_26 ];
            dontConfigure = true;
            env = {
              CGO_ENABLED = 0;
              GOWORK = "auto";
            };
            doCheck = false;
            buildPhase = ''
              runHook preBuild
              export HOME="$TMPDIR"
              export GOPATH="$TMPDIR/go"
              export GOMODCACHE="$GOPATH/pkg/mod"
              export GOCACHE="$TMPDIR/go-cache"
              mkdir -p "$GOPATH/pkg"
              cp -R ${workspaceModuleCache} "$GOMODCACHE"
              chmod -R u+w "$GOMODCACHE"
              export GOPROXY=off
              export GOSUMDB=off

              cp -R "$src"/. work
              chmod -R u+w work

              (
                cd work/typescript-go/internal/diagnostics
                go run generate.go -diagnostics ./diagnostics_generated.go -loc ./loc_generated.go -locdir ./loc
              )

              (
                cd work
                go build -mod=readonly -trimpath -ldflags="-s -w" -o tsgo ./typescript-go/cmd/tsgo
              )
              runHook postBuild
            '';
            installPhase = ''
              runHook preInstall
              install -Dm755 work/tsgo $out/bin/tsgo
              runHook postInstall
            '';
          };

          effectTsgo = pkgs.symlinkJoin {
            name = "effect-tsgo";
            paths = [ tsgo ];
            nativeBuildInputs = [ pkgs.makeWrapper ];
            postBuild = ''
              # tsgo shells out to npm for typings acquisition in LSP mode.
              wrapProgram $out/bin/tsgo \
                --prefix PATH : ${lib.makeBinPath [ pkgs.nodejs ]}

              makeWrapper $out/bin/tsgo $out/bin/effect-tsgo \
                --add-flags "--lsp --stdio"
            '';
            meta = {
              description = "Self-contained Effect Language Service binary built on TypeScript-Go";
              license = lib.licenses.mit;
              mainProgram = "effect-tsgo";
              platforms = supportedSystems;
            };
          };
        in
        {
          default = effectTsgo;
          effect-tsgo = effectTsgo;
          inherit tsgo;
        }
      );

      apps = forAllSystems (
        system: _pkgs:
        let
          package = self.packages.${system}.effect-tsgo;
        in
        {
          default = {
            type = "app";
            program = "${package}/bin/effect-tsgo";
          };
          effect-tsgo = {
            type = "app";
            program = "${package}/bin/effect-tsgo";
          };
        }
      );

      checks = forAllSystems (
        system: _pkgs: {
          inherit (self.packages.${system}) effect-tsgo;
        }
      );
    };
}
