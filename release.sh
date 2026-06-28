#!/usr/bin/env bash
# release.sh — bumpa la versione, aggiorna il tag, il PKGBUILD e pusha tutto
set -e

VERSION="${1:-}"
if [ -z "$VERSION" ]; then
    echo "Uso: ./release.sh 0.2.0"
    exit 1
fi

echo "==> Release v$VERSION"

# 1. Aggiorna versione in Cargo.toml
sed -i "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml

# 2. Aggiorna versione in PKGBUILD
sed -i "s/^pkgver=.*/pkgver=$VERSION/" PKGBUILD

# 3. Commit e push del codice
git add -A
git commit -m "Release v$VERSION"
git push origin main

# 4. Sposta il tag
git tag -d "v$VERSION" 2>/dev/null || true
git push origin ":refs/tags/v$VERSION" 2>/dev/null || true
git tag "v$VERSION"
git push origin "v$VERSION"

# 5. Calcola nuovo sha256sum e aggiorna PKGBUILD
echo "==> Attendo che GitHub generi il tarball..."
sleep 5
SHA=$(curl -sL "https://github.com/deddroid/rust-discord-overlay/archive/refs/tags/v$VERSION.tar.gz" | sha256sum | cut -d' ' -f1)
echo "==> sha256sum: $SHA"
sed -i "s/sha256sums=('[a-f0-9]*')/sha256sums=('$SHA')/" PKGBUILD

# 6. Push del PKGBUILD aggiornato
git add PKGBUILD Cargo.toml Cargo.lock
git commit -m "Update PKGBUILD sha256sum for v$VERSION"
git push origin main

echo ""
echo "✓ Release v$VERSION completata!"
echo "  Per installare: rm -f rust-discord-overlay-*.tar.gz && makepkg -si"
