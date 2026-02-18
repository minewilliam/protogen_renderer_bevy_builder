# Project builder for Bevy on ARM64
> This utility **runs exclusively on Linux/WSL** because it wraps command-line utilities.

A Rust utility designed to simplify cross-compiling and deploying a Bevy application to ARM64 targets, such as a Raspberry Pi. This tool is intended for use with the project [protogen_renderer_bevy](https://github.com/minewilliam/protogen_renderer_bevy).

It automates the steps of:
- (Not yet) Building a `podman` container with a cross-compilation toolchain.
- Cross-compiling a Bevy project for ARM64 using `cross-rs`.
- Setting up passwordless SSH with the target device.
- Deploying the compiled binary to the target device.

Picture this project as a sort of Makefile/helper for the parent project `protogen_renderer_bevy`.

## Usage
To cross-compile and upload the binaries to your target device, simply run: 
```shell
cargo-deploy
```

To compile a debug build, add the `--debug` flag:
```shell
cargo-deploy [--debug]
```

## Dependencies

### Cross (cross-rs) – Handles cross-compilation for ARM64.
Installation:
```shell
cargo install cross
```

### Podman – Required by cross-rs for containerized builds.
If you also have `Docker` installed, it may be necessary to force `cross` to use `podman` like such:
```shell
export CROSS_CONTAINER_ENGINE=podman
```
`podman` is prefered for our use-case for being rootless and daemonless.

Use your package manager:
- Debian (aptitude):
```shell
sudo apt install podman
```
- Arch (pacman):
```shell
sudo pacman -S podman
```

### SSH and SCP
Required for connecting to the remote device and transferring the binary.
Standard OpenSSH client is expected to be installed.

### ssh-copy-id
Required for first-time SSH key setup on the remote device.


## Configuration (cargo_deploy.json)
The utility expects a `cargo_deploy.json` file in the root of your repository. It will create a default configuration file if none is found. A typical configuration looks like this:

```json
{
  "target_arch": "aarch64-unknown-linux-gnu",
  "target_dest": "/home/<username>/bin",
  "target_name": <hostname or ip>,
  "target_user": <username>
}
```

### Configuration Fields
- `target_arch`

    The Rust compilation target. Defaults to `aarch64-unknown-linux-gnu` for ARM64 devices. Only `aarch64-unknown-linux-gnu` is currently supported.

- `target_dest`
    
    The path on the remote device where the executable will be copied. The home directory (`/home/<username>/bin`) will be deduced from the `target_user` value during the initial setup process.

- `target_name`
    
    The hostname or IP address of the target device.

- `target_user`

    The username used to log in via SSH.

## Outputs
Here is a list of files/outputs to expect from running this utility on your host machine.
- Configuration file: `protogen_renderer_bevy/cargo_deploy.json`
- Template docker image: `ghcr.io/cross-rs/aarch64-unknown-linux-gnu:latest`
- Compilation toolchain docker image: `localhost/bevy_pi:latest`
- SSH key (if needed): `~/.ssh/id_ed25519_<target_user>_<target_name>`
