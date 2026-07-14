# auxv.org symbol triage

Goal: dynamically link and run the auxv.org rocket server (`~/git/auxv-dot-org`) under miros.
This document triages the GLIBC symbols miros must export, ordered by what a minimal
`--http-only` boot (no `--analytics-password`) actually calls.

## Method

- Binary: `~/git/auxv-dot-org/target/release/auxv-dot-org` (release, LTO, `strip = true`).
  `.dynsym` survives stripping, so the undefined-import list is complete.
- Symbol diff (from miros README):
  `comm -23 <(nm -D --undefined-only $BINARY | grep '@GLIBC' | ...) <(nm -D --defined-only libmiros.so | ...)`
- 206 undefined GLIBC symbols total; miros already exports 40; **166 missing**.
- Ordering evidence: `strace -f -ttt` of a real boot on port 18080, killed after a `curl`
  returned HTTP 200. `listen()` at trace line 6322 is the boot→serve boundary.
- Structural fact (`main.rs:55`): with no `--analytics-password`, `analytics = None`, so the
  entire rusqlite → sqlcipher → openssl branch is dead code — linked but never called.
  Confirmed: `shmget`/`shmat`/`uname`/`connect`/`mlock` fire 0 syscalls.
- Time functions (`clock_gettime`, `gettimeofday`) go through the vDSO and are invisible to
  strace; marked from the call graph instead.

miros already exports (relevant to boot): `pthread_create` `pthread_join`
`pthread_key_create/delete` `pthread_getspecific/setspecific` `gettid` `clone` `mmap`
`munmap` `mprotect` `mremap` `read` `write` `close` `open64` `getrandom` `syscall`
`malloc`/`calloc`/`realloc`/`free` `memcpy`/`memset`/`memmove`/`memcmp` `strlen`.

---

## Phase 1 — Rust runtime + tokio multi-thread runtime (before `main` body)

`#[rocket::main]` builds the tokio runtime first; it spawns 9 worker threads (`clone3` ×9)
that immediately lock and park. Nothing reaches user code until this stands up.

| Missing symbol | Backed by | Trace line |
|---|---|---|
| `sigaction` `signal` `sigaltstack` | `rt_sigaction`, `sigaltstack` | L50, L65 — SIGSEGV stack-overflow guard |
| `sched_getaffinity` | same | L63 — tokio worker count |
| `epoll_create1` `eventfd` `epoll_ctl` | `epoll_create1` `eventfd2` `epoll_ctl` | L99–101 — tokio reactor |
| `fcntl64` | `fcntl` | L102 — nonblocking fds |
| `socketpair` | same | L103 |
| `madvise` | same | L109 — allocator |
| `poll` | same | L49 |
| `getcwd` | same | L93 |
| `sched_yield` | same | L236 |
| `pthread_mutex_{init,destroy,lock,trylock,unlock}` · `pthread_mutexattr_{init,destroy,settype}` · `pthread_cond_{init,destroy,wait,signal,broadcast}` · `pthread_rwlock_{init,destroy,rdlock,wrlock,unlock}` · `pthread_once` · `pthread_self` · `pthread_detach` · `pthread_attr_{init,destroy,setdetachstate,setstacksize,getstack,getguardsize}` · `pthread_getattr_np` | `futex`, `clone3` | L111, L322 — **the gate**; tokio can't run without these |
| `pthread_setname_np` | `prctl` (PR_SET_NAME) | L131 |
| `clock_gettime` `gettimeofday` | vDSO (invisible) | tokio timer wheel — certain |

Always-on string/mem funcs no trace shows but every Rust instruction needs — implement first,
unconditionally: `memchr` `strcmp` `strncmp` `strcpy` `strncpy` `strchr` `strrchr` `strcspn`
`strspn`.

### Verified by gdb `dprintf` (boot + 2 requests, port 18081)

The trace can't see memory-only funcs, so a `dprintf`-probed boot resolved them empirically:

- **HOT** — `pthread_mutex_lock` (44×) · `pthread_self` (19×) · `pthread_attr_init` (19×) ·
  `pthread_getattr_np` (10×) · `pthread_setname_np` (9×) · `pthread_attr_setstacksize` (9×) ·
  `pthread_create` (9×, already exported) · `getauxval` (11×) · `sysconf` (1×).
- **Zero hits but keep for correctness** — `pthread_mutex_init` `pthread_cond_*` `pthread_rwlock_*`
  `pthread_once`. The mutexes are **statically initialized** (`PTHREAD_MUTEX_INITIALIZER` = all-zero
  blob) and never explicitly `init`'d. **Load-bearing invariant: a zeroed `pthread_mutex_t` blob
  must be a valid unlocked mutex** — no lazy-init handshake to lean on.
- **Confirmed cold** (0 hits) — `getaddrinfo` `__ctype_b_loc` `qsort` `dlopen` `pthread_detach`
  `geteuid` `getentropy`.
- **Correction** — `fopen` fired once at boot (glibc internals), so the `FILE*` layer isn't 100%
  cold even without `--analytics-password`. Minor.
- `getauxval`/`sysconf` read the auxv from memory (no syscall) — fold into the wrapper work; miros
  already has the parsed auxv.

## Phase 2 — `pages::set_page_cache()` (`main.rs:53`)

Walks `pages/` and reads every markdown + emoji SVG (traced opening `./pages/emojis/*.svg`).

| Missing symbol | Backed by | Trace line |
|---|---|---|
| `opendir` `closedir` `readdir` `readdir64` `dirfd` | `getdents64` | L525 |
| `stat` `fstat` `lstat64` `statx` | `statx`, `fstat` | L81 |
| `open` | `openat` | L4 (miros has `open64`, not bare `open`) |
| `lseek64` | `lseek` | L60 |
| `strtol` `strtok` `qsort` | — | markdown/emoji parsing (uncertain — verify) |

## Phase 3 — reach "listening" (`main.rs:64`, `TcpListener::bind`)

Fires late (L6319) because tokio starts first.

`socket` (L6319) · `setsockopt` (L6320) · `bind` (L6321) · `listen` (L6322) · `getsockname` (L6325)

## Phase 4 — serve one HTTP request

Everything past L6322, needed for the 200 from `curl`:

`accept4` (L6374) · `getpeername` (L6391) · `recv` (recvfrom, L6394) · `writev` (L6396) ·
`shutdown` (L6402) · `send` (sendto, L6450)

---

## Cold on this path — linked but never called (verified)

Not needed to serve auxv.org over HTTP:

- **sqlcipher/openssl/rusqlite FILE\* + ctype** (whole `FILE*` subsystem, gated behind
  `--analytics-password`): `fopen` `fopen64` `fclose` `fread` ~~`fwrite`~~ `fseek` `ftell` `feof`
  `ferror` ~~`fflush`~~ `fgets` ~~`fputc`~~ ~~`fputs`~~ `fileno` ~~`fprintf`~~ ~~`vfprintf`~~ `perror`
  `__ctype_b_loc` `__ctype_tolower_loc` `__isoc23_sscanf` `__isoc23_strtol` `__isoc23_strtoul`
  `posix_memalign`
- **SysV shm**: `shmget` `shmat` `shmdt` — 0 syscalls
- **DNS** (binding to `0.0.0.0` resolves nothing): `getaddrinfo` `freeaddrinfo` `gai_strerror`
  `__res_init` — `connect` fired 0 times
- **fs mutations**: `mkdir` `rmdir` `rename` `unlink` `readlink` `realpath` `ftruncate64`
  `fsync` `fchmod` `fchown` `utimes` `pwrite64` `dup` ~~`isatty`~~
- **panic/unwind/shutdown**: `dl_iterate_phdr` `dladdr` `dlopen` `dlclose` `dlsym` `dlerror`
  `setcontext` `_setjmp` `_longjmp` ~~`__cxa_atexit`~~ `__cxa_finalize` ~~`exit`~~ `__assert_fail`
- **misc**: `uname` `gnu_get_libc_version` `__libc_current_sigrtmax` `mlock` `munlock` `log`
  `pow` `nanosleep` `pause` `localtime_r` `strftime` `secure_getenv` `geteuid`

## Can't confirm from strace — verify before assuming

Read auxv/memory (no syscall), so the trace is blind. A couple may be Phase 1:
`getauxval` `sysconf` `getentropy`. Treat `getauxval`/`sysconf` as probably boot-critical.

---

## Bottom line

~55 symbols across Phases 1–4 get from exec to a served 200. The bulk is one push on
**pthread mutex/cond/rwlock** (Phase 1) — tokio is the gate. The remaining ~110 missing symbols
are dead code behind `--analytics-password` or panic/teardown paths.
