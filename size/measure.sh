#!/bin/sh
# Builds hashsize for every target x profile x implementation and prints, per
# target, a markdown table of what each BLAKE3 implementation adds over the
# baseline. See README.md.
#
# Filters (space-separated lists; defaults = everything):
#   TARGETS  x86_64-unknown-uefi-sse2 x86_64-unknown-uefi aarch64-unknown-uefi x86_64-unknown-none
#   IMPLS    blakity reference blake3 blake3-pure c      (the baseline is always built)
#   LTO      off false thin fat                          (profile.size.lto; default skips
#                                                         false, which matches off here)
#   OPT      1 2 3 s z                                   (profile.size.opt-level; default skips 1)
#   HASH_OPT same 1 2 3 s z                              (opt-level of the hash crate only;
#                                                         same = OPT; default "same 3")
#   OUTLINER 0 1                                         (1 = -Cllvm-args=-enable-machine-outliner=always)
#   BREAD    1 to add the bread-exact row, 0 to skip it
#   JOBS     targets built concurrently (default 4; each target has its own target dir)
# Raw numbers go to results/<target>.tsv; the tables go to stdout and
# results/sizes.md. Each target's last cargo output is in target/<target>/build.log.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
cd "$here"

# The pinned nightly, llvm-size and clang come from the repo's dev shell
# (flake.nix sets LLVM_SIZE and HASHSIZE_CC); re-enter it if needed.
if [ -z "${HASHSIZE_CC:-}" ] || [ -z "${LLVM_SIZE:-}" ]; then
    exec nix develop "$here/.." -c "$here/measure.sh" "$@"
fi

TARGETS=${TARGETS:-"x86_64-unknown-uefi-sse2 x86_64-unknown-uefi aarch64-unknown-uefi x86_64-unknown-none"}
IMPLS=${IMPLS:-"blakity reference blake3 blake3-pure c"}
LTO=${LTO:-"off thin fat"}
OPT=${OPT:-"2 3 s z"}
HASH_OPT=${HASH_OPT:-"same 3"}
OUTLINER=${OUTLINER:-"0 1"}
BREAD=${BREAD:-1}
JOBS=${JOBS:-4}

die() { echo "measure.sh: $*" >&2; exit 1; }

[ -x "$LLVM_SIZE" ] || die "LLVM_SIZE=$LLVM_SIZE is not executable"
[ -x "$HASHSIZE_CC" ] || die "HASHSIZE_CC=$HASHSIZE_CC is not executable"
# impl-c and the blake3 crate's non-pure C/asm backends build with it.
export TARGET_CC="$HASHSIZE_CC"

mkdir -p results

target_arg() {
    case $1 in
        x86_64-unknown-uefi-sse2) echo "$here/targets/$1.json" ;;
        *) echo "$1" ;;
    esac
}

# Extra C flags matching each Rust target's ABI (c-shim stands in for libc headers).
target_cflags() {
    f="-isystem $here/c-shim"
    case $1 in
        x86_64-unknown-uefi) f="$f -mno-red-zone -mgeneral-regs-only" ;;
        x86_64-unknown-uefi-sse2) f="$f -mno-red-zone" ;;
        x86_64-unknown-none) f="$f -mno-red-zone -mgeneral-regs-only -fPIC" ;;
    esac
    echo "$f"
}

# build TARGET IMPL PROFILE [cargo args...] -> prints "file text ro data"
build() {
    t=$1 impl=$2 profile=$3
    shift 3
    feat=""
    [ "$impl" = none ] || feat="--features impl-$impl"
    bsf=optimize_for_size
    [ "$t" = x86_64-unknown-none ] && bsf="$bsf,compiler-builtins-mem"
    log="$here/target/$t/build.log"
    mkdir -p "$here/target/$t"
    # shellcheck disable=SC2086
    if ! CARGO_TARGET_DIR="$here/target/$t" TARGET_CFLAGS="$(target_cflags "$t")" \
        cargo build --locked --quiet --profile "$profile" --target "$(target_arg "$t")" \
        -Zbuild-std=core,compiler_builtins -Zbuild-std-features="$bsf" \
        -Zjson-target-spec -Zprofile-rustflags $feat "$@" >"$log" 2>&1; then
        cat "$log" >&2
        die "build failed: target=$t impl=$impl profile=$profile $*"
    fi
    bin="$here/target/$t/$t/$profile/hashsize"
    [ "${t#*uefi}" != "$t" ] && bin="$bin.efi"
    [ -f "$bin" ] || die "missing $bin"
    file=$(wc -c <"$bin")
    "$LLVM_SIZE" -A "$bin" | awk -v file="$file" '
        $1 == ".text" { t += $2 }
        $1 == ".rdata" || $1 == ".rodata" { r += $2 }
        $1 == ".data" { d += $2 }
        END { printf "%d %d %d %d\n", file, t, r, d }'
}

toml_lto() { case $1 in off|thin|fat) echo "\"$1\"" ;; *) echo "$1" ;; esac; }
toml_opt() { case $1 in s|z) echo "\"$1\"" ;; *) echo "$1" ;; esac; }

# The crate that holds an implementation's code, for the HASH_OPT override.
hash_crate() {
    case $1 in
        blakity) echo blakity ;;
        reference | c) echo upstream ;;
        blake3 | blake3-pure) echo blake3 ;;
    esac
}

measure_target() {
    t=$1
    tsv="results/$t.tsv"
    printf 'lto\topt\thash_opt\toutliner\timpl\tfile\ttext\tro\tdata\n' >"$tsv"
    # bread first: it is the configuration that ships.
    if [ "$BREAD" = 1 ]; then
        for impl in none $IMPLS; do
            echo "  $t bread $impl" >&2
            r=$(build "$t" "$impl" bread) || exit 1
            # shellcheck disable=SC2086
            set -- $r
            printf 'bread\tz\t3\t1\t%s\t%s\t%s\t%s\t%s\n' "$impl" "$@" >>"$tsv"
        done
    fi
    for lto in $LTO; do
        for opt in $OPT; do
            for hopt in $HASH_OPT; do
                # Overriding the hash crate to the global opt-level is a duplicate row.
                [ "$hopt" = "$opt" ] && continue
                for outl in $OUTLINER; do
                    flags=""
                    [ "$outl" = 1 ] && flags='"-Cllvm-args=-enable-machine-outliner=always"'
                    for impl in none $IMPLS; do
                        echo "  $t lto=$lto opt=$opt hash=$hopt outliner=$outl $impl" >&2
                        set -- --config "profile.size.lto=$(toml_lto "$lto")" \
                            --config "profile.size.opt-level=$(toml_opt "$opt")" \
                            --config "profile.size.rustflags=[$flags]"
                        if [ "$hopt" != same ] && [ "$impl" != none ]; then
                            set -- "$@" --config \
                                "profile.size.package.$(hash_crate "$impl").opt-level=$(toml_opt "$hopt")"
                        fi
                        r=$(build "$t" "$impl" size "$@") || exit 1
                        # shellcheck disable=SC2086
                        set -- $r
                        printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
                            "$lto" "$opt" "$hopt" "$outl" "$impl" "$@" >>"$tsv"
                    done
                done
            done
        done
    done
}

# One row per profile; baseline in absolute bytes, impls as deltas.
# "sections" = .text + .rdata/.rodata + .data; file deltas move in FileAlignment
# (512 B) steps on PE.
render() {
    awk -F'\t' -v target="$1" -v impls="$IMPLS" '
        function comma(n,  s, neg) {
            neg = n < 0; if (neg) n = -n; s = ""
            while (n >= 1000) { s = sprintf(",%03d", n % 1000) s; n = int(n / 1000) }
            return (neg ? "-" : "") n s
        }
        function sgn(n) { return (n > 0 ? "+" : "") comma(n) }
        NR == 1 { next }
        {
            key = $1 SUBSEP $2 SUBSEP $3 SUBSEP $4
            if (!(key in seen)) {
                seen[key] = 1; order[++rows] = key
                lto[key] = $1; opt[key] = $2; hopt[key] = $3; outl[key] = $4
            }
            sec[key, $5] = $7 + $8 + $9; file[key, $5] = $6
        }
        END {
            n = split(impls, im, " ")
            printf "### %s\n\nBaseline: absolute bytes, sections (file). Implementations: delta vs baseline, sections (file).\n\n", target
            printf "| lto | opt | hash opt | outliner | baseline |"; for (i = 1; i <= n; i++) printf " %s |", im[i]; printf "\n"
            printf "| --- | --- | --- | --- | ---: |"; for (i = 1; i <= n; i++) printf " ---: |"; printf "\n"
            for (r = 1; r <= rows; r++) {
                k = order[r]
                if (lto[k] == "bread") printf "| **bread** (fat) | z | 3 | on |"
                else printf "| %s | %s | %s | %s |", lto[k], opt[k], hopt[k], (outl[k] == 1 ? "on" : "off")
                printf " %s (%s) |", comma(sec[k, "none"]), comma(file[k, "none"])
                for (i = 1; i <= n; i++)
                    printf " %s (%s) |", sgn(sec[k, im[i]] - sec[k, "none"]), sgn(file[k, im[i]] - file[k, "none"])
                printf "\n"
            }
            printf "\n"
        }' "$2"
}

# Run targets concurrently (JOBS at a time), each in its own target dir.
status=0
pids=""
running=0
for t in $TARGETS; do
    measure_target "$t" &
    pids="$pids $!"
    running=$((running + 1))
    if [ "$running" -ge "$JOBS" ]; then
        for p in $pids; do wait "$p" || status=1; done
        pids=""
        running=0
    fi
done
for p in $pids; do wait "$p" || status=1; done
[ "$status" = 0 ] || die "some builds failed (see above)"

{
    cat <<EOF
## BLAKE3 size matrix

rustc: $(rustc -V)
**bread** (first row, the configuration that ships): bread's release profile (fat LTO, opt-level z, hash crate at 3, \`-Zfmt-debug=none -Cllvm-args=-enable-machine-outliner=always\`).
Other rows: profile \`size\` (codegen-units=1, panic=abort, strip, trim-paths) with lto, opt-level, the hash crate's opt-level (\`same\` = opt) and the machine outliner swept.

EOF
    for t in $TARGETS; do render "$t" "results/$t.tsv"; done
} | tee results/sizes.md
