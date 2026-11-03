#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "$0")/.."
dest=$(realpath -m "${1:?Destination required}")
mkdir -p "$dest"
for project in gui plugin; do
    case "$project" in
        gui) repo=minifuse-control ;;
        plugin) repo=omarchy-minifuse ;;
    esac
    sha=$(python3 -c 'import json,sys; print(json.load(open("distribution/release.json"))[sys.argv[1]])' "$project")
    [[ $sha =~ ^[0-9a-f]{40}$ ]]
    git clone --no-checkout "https://github.com/srdiogo/$repo.git" "$dest/$project"
    git -C "$dest/$project" merge-base --is-ancestor "$sha" origin/main
    git -C "$dest/$project" checkout --detach "$sha"
done
