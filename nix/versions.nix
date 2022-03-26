# declares all pinned versions of packages we are using during the build
{ rocksDBVersion ? "6.29.3" }:
rec {
  fetchImportCargoLock = builtins.fetchTarball {
    url = "https://github.com/NixOS/nixpkgs/archive/be872a7453a176df625c12190b8a6c10f6b21647.tar.gz";
    sha256 = "1hnwh2w5rhxgbp6c8illcrzh85ky81pyqx9309bkgpivyzjf2nba";
  };

  importCargoLock = (import fetchImportCargoLock {}).rustPlatform.importCargoLock;

  fetchCrate2nix = builtins.fetchTarball {
    url = "https://github.com/NixOS/nixpkgs/archive/c82b46413401efa740a0b994f52e9903a4f6dcd5.tar.gz";
    sha256 = "13s8g6p0gzpa1q6mwc2fj2v451dsars67m4mwciimgfwhdlxx0bk";
  };

  crate2nix = (import fetchCrate2nix {}).crate2nix;

  # pinned version of nix packages
  # main reason for not using here the newest available version at the time or writing is that this way we depend on glibc version 2.31 (Ubuntu 20.04 LTS)
  fetchNixpkgs = (builtins.fetchTarball {
    url = "https://github.com/NixOS/nixpkgs/archive/2c162d49cd5b979eb66ff1653aecaeaa01690fcc.tar.gz";
    sha256 = "08k7jy14rlpbb885x8dyds5pxr2h64mggfgil23vgyw6f1cn9kz6";
  });

  # this overlay allows us to use a specified version of the rust toolchain
  fetchRustOverlay = builtins.fetchTarball {
    url = "https://github.com/mozilla/nixpkgs-mozilla/archive/15b7a05f20aab51c4ffbefddb1b448e862dccb7d.tar.gz";
    sha256 = "0admybxrjan9a04wq54c3zykpw81sc1z1nqclm74a7pgjdp7iqv1";
  };

  nixpkgs =
    let
      # this overlay allows us to use a specified version of the rust toolchain
      rustOverlay =
        import fetchRustOverlay;

      overrideRustTarget = rustChannel: rustChannel // {
        rust = rustChannel.rust.override {
          targets = [ "x86_64-unknown-linux-gnu" "wasm32-unknown-unknown" ];
        };
      };
      rustToolchain = with nixpkgs; overrideRustTarget ( rustChannelOf { rustToolchain = ../rust-toolchain; } );

      inherit crate2nix;

      # pinned version of nix packages
      nixpkgs = import fetchNixpkgs { overlays = [
            rustOverlay
            (self: super: {
              inherit (rustToolchain) cargo rust-src rust-std;
              rustc = rustToolchain.rust;

              inherit crate2nix;
            })
          ];
        };
    in
    nixpkgs;

  fetchDockerNixpkgs = builtins.fetchTarball {
    url = "https://github.com/NixOS/nixpkgs/archive/be872a7453a176df625c12190b8a6c10f6b21647.tar.gz";
    sha256 = "1hnwh2w5rhxgbp6c8illcrzh85ky81pyqx9309bkgpivyzjf2nba";
  };

  dockerNixpkgs = import fetchDockerNixpkgs {};

  fetchGitignoreSource = nixpkgs.fetchFromGitHub {
    owner = "hercules-ci";
    repo = "gitignore.nix";
    rev = "5b9e0ff9d3b551234b4f3eb3983744fa354b17f1";
    sha256 = "o/BdVjNwcB6jOmzZjOH703BesSkkS5O7ej3xhyO8hAY=";
  };

  gitignoreSource = (import fetchGitignoreSource { inherit (nixpkgs) lib; }).gitignoreSource;

  fetchRocksdb = builtins.fetchGit {
    url = "https://github.com/facebook/rocksdb.git";
    ref = "refs/tags/v${rocksDBVersion}";
  };

  # we use a newer version of rocksdb than the one provided by nixpkgs
  # we disable all compression algorithms and force it to use SSE 4.2 cpu instruction set
  customRocksdb = nixpkgs.rocksdb.overrideAttrs (attrs: {

    src = fetchRocksdb;

    version = "${rocksDBVersion}";

    patches = [];

    cmakeFlags = [
       "-DPORTABLE=0"
       "-DWITH_JEMALLOC=1"
       "-DWITH_JNI=0"
       "-DWITH_BENCHMARK_TOOLS=0"
       "-DWITH_TESTS=0"
       "-DWITH_TOOLS=0"
       "-DWITH_BZ2=0"
       "-DWITH_LZ4=0"
       "-DWITH_SNAPPY=0"
       "-DWITH_ZLIB=0"
       "-DWITH_ZSTD=0"
       "-DWITH_GFLAGS=0"
       "-DUSE_RTTI=0"
       "-DFORCE_SSE42=1"
       "-DROCKSDB_BUILD_SHARED=0"
    ];

    propagatedBuildInputs = [];

    buildInputs = [ nixpkgs.git nixpkgs.jemalloc ] ++ (attrs.buildInputs or []);
  });

}
