#!/usr/bin/env bash
# P4 occlusion benches (ballpark, shared box): the acceptance run, its controls and the GPU A/B.
# ./bench_occ.sh TAG [OUT]  -> OUT/<TAG>_<run>_occ.json (+ logs)
set -u
cd "$(dirname "$0")"
tag=${1:-run}
out=${2:-results}
exe=../../target/release/cityview.exe  # cargo build --release -p cityview (CITYVIEW_ASSETS: the asset root)
mkdir -p "$out"
run() { local name=$1; shift; timeout 1800 env "$@" > "$out/${tag}_${name}.log" 2>&1; echo "$name: exit $?"; }
run on        $exe --bench occ --tag ${tag}_on --out "$out"
run squad     $exe --bench occ --squad 3 --tag ${tag}_squad --out "$out"
run off       $exe --bench occ --occ off --tag ${tag}_off --out "$out"
run nohyst    CV_OCC_NO_HYST=1 $exe --bench occ --tag ${tag}_nohyst --out "$out"
run noprepass CV_OCC_NO_PREPASS_CUT=1 $exe --bench occ --tag ${tag}_noprepass --out "$out"
run noglb     CV_OCC_NO_GLB_RAYS=1 $exe --bench occ --path glb --tag ${tag}_noglb --out "$out"
run gpu       $exe --bench occ --gpu --tag ${tag}_gpu --out "$out"
run gpu_base  CV_NO_OCC=1 $exe --bench occ --gpu --occ off --tag ${tag}_gpu_base --out "$out"
