{
  description = "Self-contained Effect Language Service (TypeScript-Go)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    nixpkgsUnstable.url = "github:NixOS/nixpkgs/nixos-unstable";
    /* Source of truth: git submodule `typescript-go` commit.
       Keep in sync via `_tools/update-flake-vendor-hash.sh`. */
    typescript-go-src = {
      url = "github:microsoft/typescript-go/8b515c6ce5f821ed382cdb540c6df488738bb515?submodules=1";
      flake = false;
    };
    /* Source of truth: typescript-go's `_submodules/TypeScript` commit.
       Keep in sync via `_tools/update-flake-vendor-hash.sh`. */
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
       Hash of the vendor directory produced by `go work vendor`.
       Unlike `go mod download all` (which copies $GOMODCACHE with
       non-deterministic metadata), `go work vendor` outputs a canonical
       directory of Go source files — deterministic across all platforms.

       Refresh workflow:
       1. Run `./_tools/update-flake-vendor-hash.sh`.
       2. Commit the resulting `flake.nix` update if the script changed it.

       Manual fallback:
       1. Temporarily set this value to `lib.fakeHash`.
       2. Run `nix build .#effect-tsgo --no-write-lock-file`.
       3. Copy the reported `got: sha256-...` value back here.
      */
      vendorHash = "sha256-ROqSI2i1OtRpzxx/z97sBcqNgvNbuK3KeHJw/WTvvO4=";
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

          goVendor = pkgs.stdenvNoCC.mkDerivation {
            name = "effect-tsgo-go-vendor";
            inherit src;
            nativeBuildInputs = [ pkgsUnstable.go_1_26 ];
            env = {
              CGO_ENABLED = "0";
              GOWORK = "auto";
            };
            outputHashMode = "recursive";
            outputHash = vendorHash;
            buildPhase = ''
              runHook preBuild
              export HOME="$TMPDIR"
              export GOPATH="$TMPDIR/go"
              export GOCACHE="$TMPDIR/go-cache"

              cp -R "$src"/. work
              chmod -R u+w work

              (
                cd work
                go work vendor
              )
              runHook postBuild
            '';
            installPhase = ''
              runHook preInstall
              cp -R work/vendor $out
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
              CGO_ENABLED = "0";
              GOWORK = "auto";
            };
            doCheck = false;
            buildPhase = ''
              runHook preBuild
              export HOME="$TMPDIR"
              export GOPATH="$TMPDIR/go"
              export GOCACHE="$TMPDIR/go-cache"

              cp -R "$src"/. work
              chmod -R u+w work

              cp -R ${goVendor} work/vendor
              chmod -R u+w work/vendor

              (
                cd work/typescript-go/internal/diagnostics
                go run -mod=vendor generate.go -diagnostics ./diagnostics_generated.go -loc ./loc_generated.go -locdir ./loc
              )

              (
                cd work
                go build -mod=vendor -trimpath -ldflags="-s -w" -o tsgo ./typescript-go/cmd/tsgo
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
