# tsrs patches for 7.1.0-beta.tsrs2

`_patches/tsrs/`, patch for patch (same file names), rebased on tsrs `b12862d0636c21a98d53d01c6b29570ef5f604e7`
(npm `tsrs@7.1.0-beta.tsrs2`, the tsrs vite-plus 1.7.0 pins). `repoctl submodules setup --component tsrs --version
7.1.0-beta.tsrs2` applies this folder instead of `_patches/tsrs/`. Every change to `_patches/tsrs/` is made here too;
`repoctl check` fails when the file names differ, and when no `components.tsrs` entry of `_packages/tsgo/upstream.json`
has this gitHead (delete the folder with that tsrs).
