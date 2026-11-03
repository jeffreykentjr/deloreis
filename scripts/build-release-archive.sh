#!/usr/bin/env bash
set -euo pipefail

python3 scripts/release-control.py
cargo build --release --locked -p deloreis
target/release/deloreis --version
target/release/deloreis check examples/camelot.dli
scratch="$(mktemp -d)"
trap 'rm -r "$scratch"' EXIT
target/release/deloreis init "$scratch/story"
target/release/deloreis mcp --help > /dev/null

name="deloreis-${RELEASE_TAG#v}-${RELEASE_TARGET}"
mkdir -p "dist/$name/bin"
cp target/release/deloreis "dist/$name/bin/"
cp LICENSE README.md "dist/$name/"
python3 scripts/third-party-notices.py --target "$RELEASE_TARGET" --output "dist/$name/THIRD-PARTY-NOTICES"
rustup component add rust-docs
cp "$(rustc --print sysroot)/share/doc/rust/COPYRIGHT-library.html" "dist/$name/RUST-STANDARD-LIBRARY-NOTICES.html"
tar -C dist -czf "dist/$name.tar.gz" "$name"
python3 - <<'PY'
import hashlib, os
from pathlib import Path
name = f"deloreis-{os.environ['RELEASE_TAG'][1:]}-{os.environ['RELEASE_TARGET']}.tar.gz"
archive = Path('dist') / name
digest = hashlib.sha256(archive.read_bytes()).hexdigest()
Path(str(archive) + '.sha256').write_text(f'{digest}  {name}\n')
PY
