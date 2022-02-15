## TL;DR
```
docker build -t aleph-node/build -f docker/Dockerfile_build . && \
docker run -ti --volume=$(pwd):/node/build aleph-node/build
```
Binary will be stored at `$(pwd)/aleph-node`.

### Build
We provide a build procedure based on the `nix` package manager. There are several ways to interact with this process. Users can
install `nix` locally or interact with it using docker. We prepared a simple docker image that provides necessary tools for the
whole build process. You can attempt at reproducing the build process without using `nix` by simply installing all dependencies
described by the `shell.nix` file and following execution of its `buildPhase`.

In order to build a binary for `aleph-node` using docker we first need to install docker, i.e. in case of the Ubuntu linux 
distribution, by executing `sudo apt install docker.io` (please consult your distribution's manual describing docker installation 
procedure). Next step is to prepare our docker-image that handles the build process, by invoking:
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

## WARNING
`nix` attempts to copy whole source tree in current directory before it starts the compilation process. This includes all binary
artifacts stored in `target` directory or any other files not under git.
