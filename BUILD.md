## TL;DR
```
docker build -t aleph-node/build -f docker/Dockerfile_build . && \
docker run -ti --volume=$(pwd):/node/build aleph-node/build
```
Binary will be stored at `$(pwd)/aleph-node`.

If you have [nix][nix] installed locally, you can simply call `nix-shell` (use `--pure` if you don't want to interfere with your
system's packages, i.e. `gcc`, `clang`). It should spawn a shell with all build dependencies installed.
Inside, you can simply use `cargo build --release -p aleph-node`. Keep in mind that a binary created this way will depend on
`glibc` referenced by `nix` and not necessary default one used by your system.

### Build
#### nix-with-docker way
We provide a build procedure based on the `nix` package manager. There are several ways to interact with this process. Users can
install `nix` locally or interact with it using docker. We prepared a simple docker image that provides necessary tools for the
whole build process. You can attempt at reproducing the build process without using `nix` by simply installing all dependencies
described by the `default.nix` file and following execution of its `buildPhase`.

In order to build a binary for `aleph-node` using docker we first need to install docker, i.e. in case of the Ubuntu linux
distribution, by executing `sudo apt install docker.io` (please consult your distribution's manual describing docker
installation procedure). Next step is to prepare our docker-image that handles the build process, by invoking:
```
docker build -t aleph-node/build -f docker/Dockerfile_build .
```
Created docker-image contains all necessary native build-time dependencies of `aleph-node`, i.e. `cargo`, `clang`, etc.
One can interact with that docker-image in two ways, by using either the `nix-shell` or `nix-build` command.
`nix-shell` spawns a shell that includes all build dependencies. Within it we can simply call `cargo build`.
This way, our docker instance maintains all build artifacts inside of project's root directory, which allows to speed up
ongoing build invocations, i.e. next time one invokes `cargo build` it should take significantly less time.
```
# spawn nix-shell inside of our docker image
docker run -ti --volume=$(pwd):/node/build aleph-node/build -s
# build `aleph-node` and store it at the root of the aleph-node's source directory
cargo build --release -p aleph-node
# set the proper loader (nix related)
patchelf --set-interpreter /lib64/ld-linux-x86-64.so.2 target/x86_64-unknown-linux-gnu/release/aleph-node
```

Another way to interact with this docker image is to allow it to create for us only a single `aleph-node` binary artifact,
i.e. each time we call its build process it will start it from scratch in a isolated environment.
```
# outputs the `aleph-node` binary in current dir
docker run -ti --volume=$(pwd):/node/build aleph-node/build
```

#### `I feel lucky` way
These are build dependencies used by our process for `aleph-node`
```
binutils-2.36,1
clang-13.0.0
protobuf-3.19.0
openssl-1.1.1l
git-2.33.1
nss-cacert-3.71
pkg-config-0.29.2
rust
```
Version of the rust toolchain is specified by the `rust-toolchain.toml` file. You can use [rustup][rustup] to install a specific
version of rust, including its custom compilation targets. Using `rustup` it should set correctly a toolchain automatically while
you call rust within project's root directory. Naturally, we can try to use different versions of these dependencies,
i.e. delivered by system's default package manager, but such process might be problematic. Notice, that the `nix` based process
is not referencing any of the `gcc` compiler tools, where ubuntu's package `build-essential` already includes `gcc`. It might
influence some of the build scripts of our build dependencies and it might be necessary to carefully craft some of the build-time
related environment flags, like `CXXFLAGS` etc.

## WARNING
`nix` attempts to copy whole source tree in current directory before it starts the compilation process. This includes all binary
artifacts stored in `target` directory or any other files not under git.

[nix]: https://nixos.org/manual/nix/stable/
[rustup]: https://rustup.rs/
