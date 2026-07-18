# Build infrastructure migration: own the final link

## Why

miros is a from-scratch platform — the libc, the libm, the dynamic linker. cargo/rustc is a
build system for software that *consumes* a platform: it assumes a libc, a `crt0`, a `std`, and a
system linker sit beneath you. Every build papercut in this project is the same root cause — **the
compiler driver owns policy it has no business owning when you are the bottom of the stack.**

Currently `just build_release` funnels everything through one `cargo rustc` invocation that also
drives the final link. That monolithic link is where the policy conflicts bite, and it is
structurally unable to do the one thing that fixes the worst of them (a second, non-LTO link
stage). This doc categorizes the papercuts, splits them into *controllable* vs *LLVM-fundamental*,
lists the tools, and lays out a migration to owning the link.

Every claim below is tagged: **[V]** verified empirically this session, **[R]** researched (docs/RFC),
**[P]** proposed analysis (needs a prototype).

---

## The papercuts, categorized

### Controllable — cargo/rustc policy, dissolved by owning the link

| Papercut | Mechanism | Evidence |
|---|---|---|
| `-nostartfiles`, `-Wl,-e,_start`, `-Wl,-Bsymbolic`, `-Z tls-model=initial-exec` passed through `just` | cargo has no "freestanding artifact" concept, so entry/startup/TLS are link-args smuggled through `cargo rustc` | current `justfile` **[V]** |
| cdylib export surface is wrong — linked-in symbols vanish | rustc auto-generates `--version-script=…/list` with `local: *`, hiding everything not a crate-level `#[no_mangle]` | saw the generated `--version-script`; `local:*` hides symbols; a *second* merged version script re-exported `pow` from the staticlib **[V]** |
| static-lib symbols not in `.dynsym` | same version-script `local:*` (plus `--exclude-libs`), by design since [rust#104707] | `pow` from the `.a` only exported once added to a version script **[V]** |
| `-Z build-std` gymnastics | cargo assumes `std` is a given, not something you compile | current release recipe **[V]** |

**These are all the same problem:** rustc decides the entry point, the export list, and the link
line. Own the link and each becomes one explicit line you write once — your linker script, your
version script, your `ld` flags.

### LLVM-fundamental — cannot be flag-disabled, only worked around

| Papercut | Mechanism | Evidence |
|---|---|---|
| Hardware-satisfiable libfunc definitions (`sqrt`, `floor`, `fabs`, `ceil`, `trunc`, `round`, `rint`, `copysign`, `fma`, `fmax/fmin`, `cbrt`, `fdim`, `fmod`) are **dropped** from any LTO build | LLVM lowers `f64::sqrt` to `llvm.sqrt.f64`; nothing references the *symbol* `sqrt`, so during LTO the definition is internalized and DCE'd before the backend. Non-hardware libfuncs (`pow`, `sin`, `log`, `exp`) survive because they can't be lowered to an instruction. | debug (no-LTO) keeps `define double @sqrt`; fat **and** thin LTO drop it; `force-soft-floats`, `black_box`, `#[used]`, `#[linkage="external"]`, inline-asm body, `global_asm`, `--undefined`, `--export-dynamic-symbol` all fail **[V]**. LLVM RFC calls the late-libcall handling *"reasonable to call it a bug"*; fix is proposed *inside* LLVM; `-fno-builtin`/`#![no_builtins]` does not cure it at LTO **[R]** |

**This one is not ours to fix.** `#![no_builtins]` is the `-fno-builtin` knob and it is ignored at
LTO — confirmed empirically and consistent with the RFC. The only levers are workarounds (below).

### Language-level — orthogonal to the build system and to LLVM

| Papercut | Mechanism | Fix |
|---|---|---|
| macro/`paste` dependency to build `__miros_<name>` idents | Rust has no stable `concat_idents!` | generate the `.rs` in a build step (codegen), or accept the proc-macro dep. Not a build-system issue. |

---

## The `sqrt` problem: the three real workarounds

Because the drop is LLVM-fundamental, we pick a workaround, not a fix:

1. **Resolver alias (miros-native).** **[P, feasible]** miros *is* the dynamic linker. The staticlib
   exports non-libfunc `__miros_sqrt` (which survives LTO — **[V]**: `__miros_sqrt✓`, alias `sqrt✗`);
   miros's symbol resolver maps the ~17 libfunc names to `__miros_*` when resolving for loaded
   programs. No linker fight, no ELF surgery, no build-system change. `libmiros.so`'s dynsym carries
   `__miros_sqrt`; every program under miros gets a working `sqrt`. Cost: `libmiros.so` is not a
   drop-in libm *outside* miros — a non-goal.
2. **Two-stage link (proper ELF `sqrt`).** **[P, needs prototype]** The drop only happens *inside* the
   LTO codegen pass. So: stage 1 — LTO-codegen miros to a single relocatable `.o` (`sqrt` gone,
   `__miros_sqrt` kept); stage 2 — a **plain, non-LTO** `ld.lld` of `miros.o + alias.o`, where
   `alias.o` defines `sqrt → __miros_sqrt`. No LTO pass in stage 2 ⇒ nothing re-materializes the
   drop ⇒ the alias survives into `.dynsym`. This is the *only* path to a real `sqrt` symbol, and it
   is exactly what cargo's monolithic link cannot express. Requires `-C linker-plugin-lto` (rustc
   emits bitcode) **[R]** + driving `ld.lld` by hand; the `ld -r` merge of build-std's bitcode is the
   fiddly, unproven part.
3. **Drop LTO.** **[V]** No-LTO release keeps every symbol natively. Simplest, but forfeits cross-crate
   inlining on the allocator/linker hot paths — measure before accepting.

---

## Tools that make owning the link clean

| Tool | Role |
|---|---|
| `-C linker-plugin-lto` | rustc emits **LLVM bitcode** rlibs/objects instead of machine code, deferring LTO to the linker — the enabling flag for cross-stage LTO **[R]** |
| `ld.lld` / `-Clinker=clang -Clink-arg=-fuse-ld=lld` | LLVM linker with the LTO plugin that consumes the bitcode **[R]** |
| `--emit=obj` / `--emit=llvm-bc` | force rustc to hand back objects/bitcode rather than a linked artifact |
| `ld -r` (partial/relocatable link) | merge many objects (incl. LTO output) into one `.o` — stage 1 of the two-stage link |
| version script / `--dynamic-list` | *your* export surface, replacing rustc's generated `local:*` |
| linker script (`-T`) | entry point, section layout, `PROVIDE` symbols — replaces the `-e`/`-Bsymbolic` link-args |
| `+whole-archive` / `+bundle` native-link modifiers (stable) | control static-lib inclusion from `#[link]`/`build.rs` without raw `--whole-archive` |
| `cc` crate | assemble the tiny alias `.o` (or any asm TU) in `build.rs` |
| `xtask` (cargo-native binary) | orchestrate multi-stage builds in Rust instead of a Makefile — keeps cargo for dep resolution, moves *only the link* out |
| `nm` / `readelf` / `objcopy` | inspection and (last-resort) post-link surgery |

**The shape:** keep cargo for dependency resolution and per-crate compilation; move *only the final
link* into `xtask`. Don't rewrite the world as a Makefile — take the link, leave the rest.

---

## Build driver: xtask, and drop `just`

Cargo owns the compile — the dependency DAG, the incrementality, the fingerprinting — and it keeps
that job. What we add is a *link stage*: branching logic over computed paths (build the staticlib
LTO-off, assemble the alias `.o`, emit bitcode, `ld -r` merge, final `ld` with our scripts). That
is a **program, not a dependency graph** — and a program wants a real language, in-repo, in the
project's own tongue: **`xtask`** (a workspace binary run as `cargo xtask <cmd>`).

Rejected drivers, worst-to-best fit:

- `just` / `cargo-make` — task runners; no types, so the link logic degrades into brittle shell.
- `make` — a genuine file-DAG, but redundant with cargo's DAG, and wrapping cargo in make's syntax
  is worse ergonomically than what it replaces.
- `bazel` / `buck2` / `gn+ninja` / `cmake+corrosion` / `meson` — hermetic DAG build systems built for
  10M-line multi-language trees (Fuchsia, Android, Chrome). Two orders of magnitude of overkill for
  one `.so` + a staticlib, and they fight cargo. This is where rust-for-linux (kbuild) and Fuchsia
  (gn) live because of *tree size*, not because it's right for a focused platform lib.

The tell: the pain here is *link logic*, not *dependency tracking* — cargo already owns the latter.

**Drop `just`.** It earns nothing here and costs an external dependency:

- xtask already exists in the workspace (`cargo xtask bench`) — consolidating *removes* a tool
  rather than adding one.
- `just` is an out-of-band install; `cargo xtask` needs only the toolchain the project already
  requires. "clone + cargo" becomes the whole story.
- Recipes that carry logic — the release link, the C-example compilation with the interpreter flag,
  the coming two-stage link — belong in xtask, not shell strings.
- Recipes that don't carry logic don't need a wrapper: `cargo check` / `cargo test` *are* the commands.

End state: `cargo xtask {build, build-debug, demo, examples}` for the logic; bare `cargo check` /
`cargo test` for the rest. One orchestration mechanism, cargo-native, zero external build tooling.

Cargo-native levers worth folding in (both **unverified — confirm before relying**):

- **Artifact dependencies** (`dep = { …, artifact = "staticlib" }`) to depend on `miros_libm`'s `.a`
  natively instead of hand-passing it — likely still nightly (`-Z bindeps`).
- **A custom target spec** (JSON) governs linker flavor / crt handling / possibly the export list;
  it *might* neutralize the auto-version-script cargo-natively. Unsure which spec fields apply.

---

## Migration phases

1. **Now — unblock libm via the resolver alias (workaround #1).** No infra change. Staticlib exports
   `__miros_*`; miros's resolver aliases the libfunc names. Closes the demo and the "libm half done"
   gap immediately.
2. **Later — lift the final link into `xtask` and delete the `justfile`.** Replace the
   `cargo rustc … -- -Clink-arg=…` line with: `xtask` runs `cargo rustc … --emit=obj`/
   `-Clinker-plugin-lto`, then drives `ld.lld` with an explicit version script + linker script + the
   alias object. Migrate the remaining recipes (`examples`, `demo`) into xtask subcommands, leave
   `check`/`test` as bare cargo, and drop `just` as a dependency. Fold in every controllable papercut
   here (entry, exports, startfiles).
3. **Prototype the two-stage link (workaround #2)** inside phase 2 if a real `.dynsym` `sqrt` becomes a
   goal (e.g. running programs under a stock loader for testing). Validate the `ld -r` bitcode merge
   first — that's the unproven step.

## Open questions to resolve during prototyping

- Does `ld -r` over build-std + miros bitcode produce a clean single object, and does `sqrt` actually
  drop there (leaving `__miros_sqrt`) so stage 2 can re-add it? **[P]**
- LLVM/rustc version pinning: `linker-plugin-lto` needs the linker's LLVM to match rustc's **[R]**.
- Perf delta of no-LTO vs fat-LTO on the allocator benchmarks — decides whether workaround #3 is ever
  acceptable.

## Sources

- [RFC][LTO] Handling math libcalls with LTO — <https://discourse.llvm.org/t/rfc-lto-handling-math-libcalls-with-lto/84884>
- Linker-plugin-based LTO — <https://doc.rust-lang.org/rustc/linker-plugin-lto.html>
- rust#104707 "Don't leak non-exported symbols from staticlibs" — <https://github.com/rust-lang/rust/issues/104707>
- rust#96192 "Symbols from static libraries no longer included into cdylib" — <https://github.com/rust-lang/rust/issues/96192>

[rust#104707]: https://github.com/rust-lang/rust/issues/104707
