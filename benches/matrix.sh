#!/usr/bin/env bash
# Rerun the benches under every lto x opt-level combination and print a
# markdown table of MB/s (rows: profile variant, columns: bench).
#
# Usage (from anywhere, inside `nix develop .`):
#   benches/matrix.sh                     # default subset: 1, 64, 1024 KiB, one-shot per impl
#   benches/matrix.sh blakity             # any libtest name filter(s)
#   benches/matrix.sh _0004_kib _0256_kib
#
# Builds go to target/matrix so the normal target dir is not churned.
# The profile in Cargo.toml (fat LTO, opt-level 3) is the "fat / 3" row.
set -euo pipefail
cd "$(dirname "$0")/.."

if [ $# -gt 0 ]; then
    filters=("$@")
else
    # blake3 incremental is skipped: it is the same as atonce for one update.
    filters=(_0001_kib _0064_kib _1024_kib --skip bench_incremental_)
fi

export CARGO_TARGET_DIR=target/matrix
out=$(mktemp)
trap 'rm -f "$out"' EXIT

for lto in off false thin fat; do
    case $lto in off | thin | fat) lto_val="\"$lto\"" ;; *) lto_val=$lto ;; esac
    for opt in 1 2 3 s z; do
        case $opt in s | z) opt_val="\"$opt\"" ;; *) opt_val=$opt ;; esac
        echo "== lto=$lto opt-level=$opt" >&2
        cargo bench -q --bench bench \
            --config "profile.bench.lto=$lto_val" \
            --config "profile.bench.opt-level=$opt_val" \
            -- "${filters[@]}" 2>/dev/null |
            awk -v v="$lto / $opt" '/MB\/s/ {
                name = $2; sub(/^bench_/, "", name)
                mbs = $(NF-1); gsub(/,/, "", mbs)
                print v "\t" name "\t" mbs
            }' | tee -a "$out" >&2
    done
done

awk -F'\t' '
    !($1 in seen_v) { seen_v[$1] = 1; vs[++nv] = $1 }
    !($2 in seen_n) { seen_n[$2] = 1; ns[++nn] = $2 }
    { cell[$1, $2] = $3 }
    END {
        printf "| lto / opt |"; for (j = 1; j <= nn; j++) printf " %s |", ns[j]; print ""
        printf "|---|";         for (j = 1; j <= nn; j++) printf "---:|";       print ""
        for (i = 1; i <= nv; i++) {
            printf "| %s |", vs[i]
            for (j = 1; j <= nn; j++) printf " %s |", cell[vs[i], ns[j]]
            print ""
        }
    }' "$out"
