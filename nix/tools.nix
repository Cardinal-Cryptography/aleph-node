{ pkgs
, lib ? pkgs.lib
, importCargoLock
}:
let
  outputHashes = lockFile:
    let
      lockFileContent = (builtins.fromTOML (builtins.readFile lockFile)).package or [];

      toPackageId = { name, version, source, ... }:
              "${name} ${version} (${source})";

      toPackageIdForImportCargoLock = { name, version, ... }:
              "${name}-${version}";

      parseGitSource = source:
        let
          withoutGitPlus = lib.removePrefix "git+" source;
          splitHash = lib.splitString "#" withoutGitPlus;
          splitQuestion = lib.concatMap (lib.splitString "?") splitHash;
        in
        {
          url = builtins.head splitQuestion;
          rev = lib.last splitQuestion;
        };

      mkGitHash = { source, name, ... }@attrs:
        let
          gitSource = parseGitSource source;
          src = builtins.fetchGit {
            submodules = true;
            inherit (gitSource) url rev;
            allRefs = true;
          };
          hash = pkgs.runCommand "hash-of-${name}" { nativeBuildInputs = [ pkgs.nix ]; } ''
            echo -n "$(nix-hash --type sha256 ${src})" > $out
          '';
        in
        builtins.readFile hash;

      isGitSource = { source ? null, ... }:
        lib.hasPrefix "git+" source;

      packages =
        let
          packagesWithoutLocal = builtins.filter (p: p ? source) lockFileContent;
          packageById = package: { name = toPackageId package; value = package; };
          # it removes possible duplicates (it takes first occurrence)
          packagesById = builtins.listToAttrs (builtins.map packageById packagesWithoutLocal);
        in
        builtins.attrValues packagesById;

      gitPackages = builtins.filter isGitSource packages;

      packageToNamedHash = toPackageIdFun: package: { name = toPackageIdFun package; value = mkGitHash package; };

      extraHashesForImportCargoLock = builtins.listToAttrs (map (packageToNamedHash toPackageIdForImportCargoLock) gitPackages);
  in
  extraHashesForImportCargoLock;
in
{
/* allows to propagate downloaded crates to other derivations
     src: the source that is needed to build the crate, usually the
     crate/workspace root directory
     cargoLock: path to the Cargo.lock file relative to src
  */
  vendoredCargoLock = cargoLock:
    let
      crateDir = dirOf cargoLock;
      lockFileContents = builtins.readFile cargoLock;
      extraHashesForImportCargoLock = outputHashes cargoLock;
    in
    importCargoLock { inherit lockFileContents; outputHashes = extraHashesForImportCargoLock; };
}
