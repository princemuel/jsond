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

If you already have a Rust environment set up, you can use the `cargo install`
command in your local clone of the repo:

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

### Manual

#### Linux

This example is for x86_64 GNU. Replace the file name if installing for a different architecture.

```sh
wget -c https://github.com/princemuel/jsond/releases/latest/download/jsond-x86_64-unknown-linux-gnu.tar.gz \
-O - | tar xz --strip-components=1
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

Available Linux targets:

| Architecture                | Asset name                               |
| --------------------------- | ---------------------------------------- |
| x86_64 (GNU/glibc)          | `jsond-x86_64-unknown-linux-gnu.tar.gz`  |
| x86_64 (musl)               | `jsond-x86_64-unknown-linux-musl.tar.gz` |
| aarch64 / ARM64 (GNU/glibc) | `jsond-aarch64-unknown-linux-gnu.tar.gz` |

For example, on ARM64 (Raspberry Pi 4/5 64-bit, AWS Graviton, etc.):

```sh
wget -c https://github.com/princemuel/jsond/releases/latest/download/jsond-aarch64-unknown-linux-gnu.tar.gz \
-O - | tar xz --strip-components=1
```

```sh
sudo chmod +x jsond
sudo chown root:root jsond
sudo mv jsond /usr/local/bin/jsond
```

Or on musl (Alpine Linux, statically-linked containers):

```sh
wget -c https://github.com/princemuel/jsond/releases/latest/download/jsond-x86_64-unknown-linux-musl.tar.gz \
-O - | tar xz --strip-components=1
```

```sh
sudo chmod +x jsond
sudo chown root:root jsond
sudo mv jsond /usr/local/bin/jsond
```

#### macOS

This example is for Apple Silicon (aarch64). For Intel Macs, use the `x86_64-apple-darwin` asset instead.

```sh
curl -L https://github.com/princemuel/jsond/releases/latest/download/jsond-aarch64-apple-darwin.tar.gz \
| tar xz --strip-components=1
```

```sh
sudo chmod +x jsond
```

```sh
sudo chown root:wheel jsond
```

```sh
sudo mv jsond /usr/local/bin/jsond
```

> If macOS blocks the binary on first run ("cannot be opened because the developer cannot be verified"),
> clear the quarantine flag:
>
> ```sh
> xattr -d com.apple.quarantine /usr/local/bin/jsond
> ```

#### Windows

Download the `x86_64-pc-windows-msvc` archive, extract it, and place `jsond.exe` somewhere on your `PATH`.

**PowerShell:**

```powershell
Invoke-WebRequest -Uri "https://github.com/princemuel/jsond/releases/latest/download/jsond-x86_64-pc-windows-msvc.zip" -OutFile "jsond.zip"
```

```powershell
Expand-Archive -Path "jsond.zip" -DestinationPath "."
```

```powershell
New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.local\bin" | Out-Null
Move-Item -Path ".\jsond.exe" -Destination "$env:USERPROFILE\.local\bin\jsond.exe"
```

> Make sure `%USERPROFILE%\.local\bin` is on your `PATH`. Add it permanently with:
>
> ```powershell
> [Environment]::SetEnvironmentVariable("Path", "$env:Path;$env:USERPROFILE\.local\bin", "User")
> ```
>
> Then restart your terminal for the change to take effect.
