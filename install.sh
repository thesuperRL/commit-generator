#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bin_name="git-aicommit"
install_dir="${HOME}/.local/bin"
install_path="${install_dir}/${bin_name}"

cd "$root"
cargo build --release

mkdir -p "$install_dir"
cp "target/release/${bin_name}" "$install_path"
chmod +x "$install_path"

echo "Installed ${install_path}"
echo "Run: git aicommit"

if ! echo "$PATH" | tr ':' '\n' | grep -qx "$install_dir"; then
  echo "Add ${install_dir} to your PATH if git aicommit is not found."
fi
