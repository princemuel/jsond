# Installation

jsond is available for Windows, macOS and Linux.

## Instructions

### Cargo

Requires [Rust 1.84+](https://www.rust-lang.org/tools/install).

```sh
# Install the latest rust (if you don't have rust)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

#### Cargo Binstall

If you already have a Rust environment set up, you can use the `cargo install` command:

```sh
cargo binstall -y jsond
```

Cargo will build the `jsond` binary and place it in your `CARGO_INSTALL_ROOT`.
For more details on installation location see [the cargo book][cargo-book].

[cargo-book]: https://doc.rust-lang.org/cargo/commands/cargo-install.html#description

#### Cargo (git)

If you already have a Rust environment set up, you can use the `cargo install` command in your local clone of the repo:

```sh
git clone https://github.com/princemuel/jsond.git
```

```sh
cd jsond
```

```sh
cargo install --path .
```

Cargo will build the `jsond` binary and place it in `$HOME/.cargo`.

### Manual (Linux)

This example is for x86_64 GNU. replace the file names if installing for a different arch.

```sh
wget -c https://github.com/princemuel/jsond/releases/latest/download/jsond_x86_64-unknown-linux-gnu.tar.gz \
-O - | tar xz
```

```sh
sudo chmod +x jsond
```

```sh
sudo chown root:root jsond
```

```sh
sudo mv jsond /usr/local/bin/jsond
```
