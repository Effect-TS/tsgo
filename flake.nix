{
  description = "Self-contained Effect Language Service (TypeScript-Go)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    nixpkgsUnstable.url = "github:NixOS/nixpkgs/nixos-unstable";
    /* Source of truth: git submodule `typescript-go` commit.
       Keep in sync via `_tools/update-flake-vendor-hash.sh`. */
    typescript-go-src = {
      url = "github:microsoft/typescript-go/281b3d0720ab3de6d330d211705cb0383b47500b?submodules=1";
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
       Go module vendor hash for buildGoModule (proxyVendor mode).
       proxyVendor is required because this project has deps with
       mixed-case module paths (Microsoft/go-winio vs microsoft/
       typescript-go) — `go work vendor` produces different directory
       layouts on case-sensitive (Linux) vs case-insensitive (macOS)
       filesystems. The download cache uses `!` escaping for uppercase
       letters, making it deterministic across both.

       Refresh: ./_tools/update-flake-vendor-hash.sh
       Manual:  set to lib.fakeHash, build, copy the reported hash.
      */
      vendorHash = "sha256-vmu1da/dP38QbB//fhYgSXqu3nvbns31Pc1KbMWp8yU=";
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

          buildGoModule = pkgsUnstable.buildGoModule.override { go = pkgsUnstable.go_1_26; };

          tsgo = buildGoModule {
            pname = "effect-tsgo";
            version = "0.0.0";
            inherit src vendorHash;
            proxyVendor = true;
            env = {
              CGO_ENABLED = "0";
              GOWORK = "auto";
            };
            /* Prevent codegen (preBuild) from leaking into the goModules
               derivation — generate.go imports internal/repo which panics
               when compiled with -trimpath. */
            overrideModAttrs = _: { preBuild = ""; };
            preBuild = ''
              # Codegen needs repo path detection; temporarily remove -trimpath
              _saved_goflags="$GOFLAGS"
              export GOFLAGS="''${GOFLAGS//-trimpath/}"
              (
                cd typescript-go/internal/diagnostics
                go run generate.go -diagnostics ./diagnostics_generated.go -loc ./loc_generated.go -locdir ./loc
              )
              export GOFLAGS="$_saved_goflags"
            '';
            subPackages = [ "typescript-go/cmd/tsgo" ];
            ldflags = [
              "-s"
              "-w"
            ];
            doCheck = false;
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
