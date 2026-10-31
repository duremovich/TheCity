#!/usr/bin/env bash
# P3 benches (ballpark, shared box): the game camera streaming at zoom 60 and 140, the overview orbit.
# ./bench.sh [tag]  -> results/<tag>_z60_stream.json, <tag>_z140_stream.json, <tag>_overview.json (+ logs)
set -u
cd "$(dirname "$0")"
tag=${1:-run}
exe=../../target/release/cityview.exe  # cargo build --release -p cityview (CITYVIEW_ASSETS: the asset root)
mkdir -p results
for z in 60 140; do
  timeout 900 $exe --bench stream --zoom $z --speed 30 --tag ${tag}_z$z --out results > results/${tag}_z${z}.log 2>&1
  echo "stream z$z: exit $?"
done
timeout 900 $exe --bench overview --tag $tag --out results > results/${tag}_overview.log 2>&1
echo "overview: exit $?"
