# Maintainer: deddroid <deddo9412@gmail.com>
pkgname=rust-discord-overlay
pkgver=0.1.0
pkgrel=1
pkgdesc="Discord voice/text overlay for Linux, written in Rust"
arch=('x86_64')
url="https://github.com/deddroid/rust-discord-overlay"
license=('GPL3')
depends=('gtk4' 'gtk4-layer-shell' 'cairo' 'dbus')
makedepends=('rust' 'cargo' 'pkg-config')
optdepends=(
    'librsvg: for multi-size icon installation'
    'pulseaudio: for audio sync feature'
    'pipewire-pulse: for audio sync feature (PipeWire)'
)
source=("$pkgname-$pkgver.tar.gz::https://github.com/deddroid/$pkgname/archive/refs/tags/v$pkgver.tar.gz")
sha256sums=('c7ba799dfcb3586607fbbcfca1b587076899e3a49e291cba9aeebb65970f6d6c')

prepare() {
    cd "$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    cargo fetch --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
    cd "$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target
    cargo build --release
}

package() {
    cd "$pkgname-$pkgver"

    # Binary
    install -Dm755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"

    # Icon (SVG scalable)
    install -Dm644 "assets/icon.svg" \
        "$pkgdir/usr/share/icons/hicolor/scalable/apps/$pkgname.svg"

    # Generate PNG icons if librsvg is available
    if command -v rsvg-convert &>/dev/null; then
        for size in 16 32 48 64 128 256; do
            install -dm755 "$pkgdir/usr/share/icons/hicolor/${size}x${size}/apps"
            rsvg-convert -w $size -h $size "assets/icon.svg" \
                > "$pkgdir/usr/share/icons/hicolor/${size}x${size}/apps/$pkgname.png"
        done
    fi

    # Desktop file
    install -Dm644 "assets/$pkgname.desktop" \
        "$pkgdir/usr/share/applications/$pkgname.desktop"

    # README
    install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
}
