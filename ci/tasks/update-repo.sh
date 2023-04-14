#!/bin/bash

set -eu

# ----------- UPDATE REPO -----------
git config --global user.email "bot@galoy.io"
git config --global user.name "CI Bot"

pushd repo

VERSION="$(cat ../version/version)"

cat <<EOF >new_change_log.md
# [bria release v${VERSION}](https://github.com/GaloyMoney/bria/releases/tag/${VERSION})

$(cat ../artifacts/gh-release-notes.md)

$(cat CHANGELOG.md)
EOF
mv new_change_log.md CHANGELOG.md

sed -i'' "0,/version/{s/version.*/version = \"${VERSION}\"/}" Cargo.toml
sed -i'' "/^name = \"bria/,/version/{s/version.*/version = \"${VERSION}\"/}" ./Cargo.lock

git status
git add .

if [[ "$(git status -s -uno)" != ""  ]]; then
  git commit -m "ci(release): release version $(cat ../version/version)"
fi
