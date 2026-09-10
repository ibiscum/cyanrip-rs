# Update libcdio without disturbing Debian installation

How to update to libcdio 2.4.0 on debian trixie without disturbing system installation?

Debian Trixie ships **libcdio 2.2.0**, not 2.4.0, so don’t replace the distribution packages. 
Install 2.4.0 into a private prefix such as `/opt/libcdio-2.4.0`. 

## 1. Record the current state

From your project directory:

```bash
cargo tree -i libcdio-sys
cargo tree -e features -i libcdio-sys
cargo metadata --locked --no-deps
```

Check the currently available crate version:

```bash
cargo search libcdio-sys --limit 5
```

Also record Debian’s installed packages:

```bash
dpkg-query -W 'libcdio*' 2>/dev/null
pkg-config --modversion libcdio 2>/dev/null || true
```

Save the current build state before changing anything:

```bash
git add Cargo.toml Cargo.lock
git commit -m "Save state before updating libcdio-sys"
```

## 2. Install build prerequisites

Do not remove Debian’s existing `libcdio` packages.

```bash
sudo apt update
sudo apt install \
  build-essential \
  pkg-config \
  autoconf \
  automake \
  libtool \
  libpopt-dev \
  libncurses-dev \
  texinfo \
  help2man \
  wget
```

If the project uses CDDA functionality, also install the Debian development package for the relevant APIs:

```bash
sudo apt install libcdio-cdda-dev
```

If that package name is unavailable, inspect the available packages:

```bash
apt search '^libcdio'
```

## 3. Build libcdio 2.4.0 in a private prefix

Use `/opt/libcdio-2.4.0`; this will not overwrite `/usr/lib` or `/usr/include`.

```bash
mkdir -p ~/src
cd ~/src

wget https://ftp.gnu.org/gnu/libcdio/libcdio-2.4.0.tar.gz
tar xf libcdio-2.4.0.tar.gz
cd libcdio-2.4.0

./configure \
  --prefix=/opt/libcdio-2.4.0 \
  --libdir=/opt/libcdio-2.4.0/lib \
  --disable-static

make -j"$(nproc)"
make check
sudo make install
```

If `make check` fails because the machine has no optical drive, that does not necessarily indicate a library-build failure. Review the failure and continue only if the actual compilation and installation completed successfully.

Check the private installation:

```bash
find /opt/libcdio-2.4.0 -type f | sort
```

## 4. Make only this build use the private library

First test `pkg-config`:

```bash
env \
  PKG_CONFIG_PATH=/opt/libcdio-2.4.0/lib/pkgconfig:/opt/libcdio-2.4.0/share/pkgconfig \
  pkg-config --modversion libcdio
```

It should print `2.4.0`.

Then build your project in an isolated environment:

```bash
env \
  PKG_CONFIG_PATH=/opt/libcdio-2.4.0/lib/pkgconfig:/opt/libcdio-2.4.0/share/pkgconfig \
  LD_LIBRARY_PATH=/opt/libcdio-2.4.0/lib \
  cargo build
```

Using `env` this way avoids changing your global shell configuration.

## 4.1 Obtain libcdio-paranoia
The corresponding upstream release is commonly named:

```text
libcdio-paranoia-10.2+2.0.2
```

Download and unpack it:

cd ~/src

```bash
wget https://ftp.gnu.org/gnu/libcdio/libcdio-paranoia-10.2+2.0.2.tar.gz
tar xf libcdio-paranoia-10.2+2.0.2.tar.gz
cd libcdio-paranoia-10.2+2.0.2
```

## 4.2 Configure it against private libcdio
Set the environment variables only for this build:

```bash
export LIBCDIO_PREFIX=/opt/libcdio-2.4.0
export PKG_CONFIG_PATH="$LIBCDIO_PREFIX/lib/pkgconfig:$LIBCDIO_PREFIX/share/pkgconfig"
export LD_LIBRARY_PATH="$LIBCDIO_PREFIX/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
```

Check the dependencies before configuring:

```bash
pkg-config --modversion libcdio
pkg-config --cflags --libs libcdio
```

Then configure:

```bash
./configure \
  --prefix=/opt/libcdio-2.4.0 \
  --libdir=/opt/libcdio-2.4.0/lib \
  --disable-static
```

Copy test data:

```bash
cd test/data
wget https://github.com/libcdio/libcdio-paranoia/raw/refs/heads/master/test/data/BOING.BIN
```

Build and test:

```bash
make -j"$(nproc)"
make check
```
Install into the same private prefix:

```bash
sudo make install
```

This should install files such as:

```bash
/opt/libcdio-2.4.0/lib/libcdio_cdda.so
/opt/libcdio-2.4.0/lib/libcdio_paranoia.so
/opt/libcdio-2.4.0/lib/pkgconfig/libcdio_cdda.pc
/opt/libcdio-2.4.0/lib/pkgconfig/libcdio_paranoia.pc
```

It may also install the cd-paranoia utility.

## 4.3. Verify both CDDA libraries

```bash
PKG_CONFIG_PATH=/opt/libcdio-2.4.0/lib/pkgconfig:/opt/libcdio-2.4.0/share/pkgconfig \
pkg-config --modversion libcdio_cdda

PKG_CONFIG_PATH=/opt/libcdio-2.4.0/lib/pkgconfig:/opt/libcdio-2.4.0/share/pkgconfig \
pkg-config --modversion libcdio_paranoia
```

Check their link flags:

```bash
PKG_CONFIG_PATH=/opt/libcdio-2.4.0/lib/pkgconfig:/opt/libcdio-2.4.0/share/pkgconfig \
pkg-config --cflags --libs libcdio_cdda libcdio_paranoia
```

Check the shared-library dependencies:

```bash
ldd /opt/libcdio-2.4.0/lib/libcdio_cdda.so
ldd /opt/libcdio-2.4.0/lib/libcdio_paranoia.so
```

They should resolve libcdio.so from:

```bash
/opt/libcdio-2.4.0/lib
```

## 4.4 Build cyanrip-rs

From the project directory:

```bash
env \
  PKG_CONFIG_PATH=/opt/libcdio-2.4.0/lib/pkgconfig:/opt/libcdio-2.4.0/share/pkgconfig \
  LD_LIBRARY_PATH=/opt/libcdio-2.4.0/lib \
  cargo clean

env \
  PKG_CONFIG_PATH=/opt/libcdio-2.4.0/lib/pkgconfig:/opt/libcdio-2.4.0/share/pkgconfig \
  LD_LIBRARY_PATH=/opt/libcdio-2.4.0/lib \
  cargo build
```

## 5. Update `libcdio-sys`

Check where libcdio-sys comes from:

```bash
cargo tree --features "backend-libcdio-sys paranoia cdda" -i libcdio-sys
```

Check the latest available crate version:

```bash
cargo search libcdio-sys --limit 1
```

Update the dependency, if libcdio-sys is a direct dependency:

```bash
cargo add libcdio-sys
```

Or specify a version explicitly in Cargo.toml, for example:

```toml
[dependencies]
libcdio-sys = "3.0"
```

Then update only that crate:

```bash
cargo update -p libcdio-sys
```
If it is transitive, update the crate shown by `cargo tree -i libcdio-sys`, or use:

```bash
cargo update -p libcdio-sys
```







If it is a direct dependency, update `Cargo.toml` to the desired version. To discover the current latest version automatically:

```bash
cargo search libcdio-sys --limit 1
```

Then update only that package:

```bash
cargo update -p libcdio-sys
```

If you need to force a particular version:

```bash
cargo update -p libcdio-sys --precise VERSION
```

Replace `VERSION` with the version you intend to use.

Check what Cargo selected:

```bash
cargo tree -i libcdio-sys
grep -A2 -B2 'name = "libcdio-sys"' Cargo.lock
```

If another dependency controls `libcdio-sys`, update that parent crate instead, or use a compatible version constraint in `Cargo.toml`.

## 6. Build and test with the private libraries

```bash
env \
  PKG_CONFIG_PATH=/opt/libcdio-2.4.0/lib/pkgconfig:/opt/libcdio-2.4.0/share/pkgconfig \
  LD_LIBRARY_PATH=/opt/libcdio-2.4.0/lib \
  cargo clean

env \
  PKG_CONFIG_PATH=/opt/libcdio-2.4.0/lib/pkgconfig:/opt/libcdio-2.4.0/share/pkgconfig \
  LD_LIBRARY_PATH=/opt/libcdio-2.4.0/lib \
  cargo test
```

For a release build:

```bash
env \
  PKG_CONFIG_PATH=/opt/libcdio-2.4.0/lib/pkgconfig:/opt/libcdio-2.4.0/share/pkgconfig \
  LD_LIBRARY_PATH=/opt/libcdio-2.4.0/lib \
  cargo build --release
```

Check which library the executable uses:

```bash
ldd target/release/your-program | grep -E 'cdio|iso9660|udf'
```

You want paths under:

```bash
/opt/libcdio-2.4.0/lib
```

rather than Debian’s `/usr/lib/...`.

## 7. Make the runtime dependency reproducible

`LD_LIBRARY_PATH` is useful for testing, but your deployed program should have a reproducible runtime path. For a local application, add an rpath during linking:

```bash
export RUSTFLAGS="-C link-arg=-Wl,-rpath,/opt/libcdio-2.4.0/lib"
```

Then rebuild:

```bash
env \
  PKG_CONFIG_PATH=/opt/libcdio-2.4.0/lib/pkgconfig:/opt/libcdio-2.4.0/share/pkgconfig \
  cargo build --release
```

Verify:

```bash
readelf -d target/release/your-program | grep -E 'RPATH|RUNPATH'
```

Alternatively, install the private library beside your application and use a relative `$ORIGIN` rpath, which is often preferable for distributing a self-contained application.

## 8. Keep the change reversible

Do not run commands such as:

```bash
sudo make install
```

from the libcdio source directory unless `./configure` was explicitly given the `/opt/libcdio-2.4.0` prefix.

To remove the private installation later:

```bash
sudo rm -rf /opt/libcdio-2.4.0
```

Restore the Rust project:

```bash
git restore Cargo.toml Cargo.lock
cargo clean
```

Debian’s original packages remain untouched throughout this process.