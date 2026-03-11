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
      workspaceModuleCacheHashes = {
        x86_64-linux = "sha256-yZucMz5/FRchCYexUrK3ZOysCk7HN56DORZa4Mo6BKo=";
        aarch64-linux = "sha256-2ch8mqutzB5UawV821qjKt8jS11R2js5RtfqN42AWXQ=";
        x86_64-darwin = "sha256-ZYeuPgm3QSKmkspvLzJIJ961NLzkOihESN8HxD77/kw=";
        aarch64-darwin = "sha256-2ch8mqutzB5UawV821qjKt8jS11R2js5RtfqN42AWXQ=";
      };
      bootstrapModuleCacheHashes = {
        x86_64-linux = "sha256-yZucMz5/FRchCYexUrK3ZOysCk7HN56DORZa4Mo6BKo=";
        aarch64-linux = lib.fakeHash;
        x86_64-darwin = lib.fakeHash;
        aarch64-darwin = lib.fakeHash;
      };
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
          bootstrapSrc = pkgs.stdenvNoCC.mkDerivation {
            name = "effect-tsgo-bootstrap-source";
            src = rootSrc;
            dontConfigure = true;
            unpackPhase = ''
              runHook preUnpack
              mkdir source
              cp -R ${rootSrc}/. source/
              chmod -R u+w source
              rm -f source/CLAUDE.md
              cp -R ${patchedTypescriptGo} source/typescript-go
              chmod -R u+w source/typescript-go
              mkdir -p source/typescript-go/_submodules
              if [ -d source/typescript-go/_submodules/TypeScript ]; then
                rmdir source/typescript-go/_submodules/TypeScript
              fi
              ln -s ${typescript-src} source/typescript-go/_submodules/TypeScript
              runHook postUnpack
            '';
            installPhase = ''
              runHook preInstall
              cp -R source $out
              chmod -R a-w $out
              runHook postInstall
            '';
          };

          bootstrapModuleCache = pkgs.stdenvNoCC.mkDerivation {
            name = "effect-tsgo-bootstrap-gomodcache";
            src = bootstrapSrc;
            nativeBuildInputs = [ pkgsUnstable.go_1_26 ];
            env = {
              CGO_ENABLED = 0;
              GOWORK = "auto";
            };
            outputHashMode = "recursive";
            outputHash = bootstrapModuleCacheHashes.${system};
            buildPhase = ''
              runHook preBuild
              export HOME="$TMPDIR"
              export GOPATH="$TMPDIR/go"
              export GOMODCACHE="$GOPATH/pkg/mod"
              mkdir -p "$GOMODCACHE"
              go mod download
              runHook postBuild
            '';
            installPhase = ''
              runHook preInstall
              cp -R "$GOMODCACHE" $out
              runHook postInstall
            '';
            dontFixup = true;
          };

          src = pkgs.stdenvNoCC.mkDerivation {
            name = "effect-tsgo-source";
            src = bootstrapSrc;
            nativeBuildInputs = [ pkgsUnstable.go_1_26 ];
            dontConfigure = true;
            unpackPhase = ''
              runHook preUnpack
              cp -R "$src" source
              chmod -R u+w source
              runHook postUnpack
            '';
            buildPhase = ''
              runHook preBuild
              export HOME="$TMPDIR"
              export GOPATH="$TMPDIR/go"
              export GOMODCACHE="$GOPATH/pkg/mod"
              mkdir -p "$GOPATH/pkg"
              cp -R ${bootstrapModuleCache} "$GOMODCACHE"
              chmod -R u+w "$GOMODCACHE"
              export GOPROXY=off
              export GOSUMDB=off
              export GOCACHE="$TMPDIR/go-cache"
              (
                cd source/typescript-go/internal/diagnostics
                go run generate.go -diagnostics ./diagnostics_generated.go -loc ./loc_generated.go -locdir ./loc
              )
              runHook postBuild
            '';
            installPhase = ''
              runHook preInstall
              cp -R source $out
              chmod -R a-w $out
              runHook postInstall
            '';
          };

          workspaceModuleCache = pkgs.stdenvNoCC.mkDerivation {
            name = "effect-tsgo-workspace-gomodcache";
            inherit src;
            nativeBuildInputs = [ pkgsUnstable.go_1_26 ];
            env = {
              CGO_ENABLED = 0;
              GOWORK = "auto";
            };
            outputHashMode = "recursive";
            outputHash = workspaceModuleCacheHashes.${system};
            buildPhase = ''
              runHook preBuild
              export HOME="$TMPDIR"
              export GOPATH="$TMPDIR/go"
              export GOMODCACHE="$GOPATH/pkg/mod"
              mkdir -p "$GOPATH/pkg"
              cp -R ${bootstrapModuleCache} "$GOMODCACHE"
              chmod -R u+w "$GOMODCACHE"
              export GOPROXY=off
              export GOSUMDB=off
              go build -trimpath -o "$TMPDIR/tsgo" ./typescript-go/cmd/tsgo
              runHook postBuild
            '';
            installPhase = ''
              runHook preInstall
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
              mkdir -p "$GOPATH/pkg"
              cp -R ${workspaceModuleCache} "$GOMODCACHE"
              chmod -R u+w "$GOMODCACHE"
              export GOPROXY=off
              export GOSUMDB=off
              go build -mod=readonly -trimpath -ldflags="-s -w" -o tsgo ./typescript-go/cmd/tsgo
              runHook postBuild
            '';
            installPhase = ''
              runHook preInstall
              install -Dm755 tsgo $out/bin/tsgo
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
