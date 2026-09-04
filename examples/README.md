# Examples

The examples demonstrate the capabilities of Miros, while also acting as e2e tests.

## Test Case Language Spec

Each test documents its expected behavior in comment directives, which the runner (`cargo xtask test`) parses and executes. A directive is a `//` comment that starts its line. Inline `//` comments and block comments are ignored.

Directives come in three kinds. `ARGS`, `STATUS`, and `NO-TTY` are file-scope: they may appear anywhere, and duplicates are an error. `STDOUT` and `STDERR` are order-sensitive: they retarget the output directives that follow them. The rest run sequentially.

### Directives

| Directive | Meaning |
|-----------|---------|
| `ARGS "s"` | Pass `s` as command-line arguments, split on whitespace. |
| `WAIT-FOR "s"` | Block until `s` appears in the output, then consume through the match. The runner appends `\n` to `s`. |
| `EXPECT "s"` | At exit, assert `s` appears in the output not consumed by `WAIT-FOR`. The runner appends `\n` to `s`. |
| `INPUT "s"` | Write `s` to stdin. |
| `SIGNAL X` | Send signal `X` to the process. `X` is an integer (1-64) or a symbolic name (`SIGTERM`). |
| `STATUS X` | Assert the exit status. `X` is an exit code (0-255), or `SIGNAL Y` for death by signal. Defaults to `STATUS 0`. |
| `STDERR` | Retarget `WAIT-FOR` and `EXPECT` to stderr for all following directives. |
| `STDOUT` | Retarget `WAIT-FOR` and `EXPECT` back to stdout (the default). |
| `NO-TTY X` | Run `X` (`STDOUT` or `STDERR`) on a pipe instead of the pseudo-terminal. |
| `EOF` | Write `^D` to stdin. The pty delivers it as EOF only when its line buffer is empty, so end `INPUT` text with `\n`. |

The process runs with stdin, stdout, and stderr attached to a pseudo-terminal. `NO-TTY` moves stdout or stderr to a pipe; stdin stays on the pseudo-terminal so `INPUT` and `EOF` keep working.

### Matching

`WAIT-FOR` directives run sequentially, each searching from where the previous one stopped. Consumed bytes are claimed. Bytes skipped become residue. At exit, `EXPECT` directives claim disjoint matches from the residue. Any byte left unclaimed on either stream fails the test.

### Grammar

```
directive := "//" ws (wait | expect | input | eof | signal | exit | stream | no-tty | args)
wait      := "WAIT-FOR" ws string
expect    := "EXPECT"   ws string
input     := "INPUT"    ws string
eof       := "EOF"
signal    := "SIGNAL"   ws (name | int)
status    := "STATUS"   ws (int | "SIGNAL" ws (name | int))
stream    := "STDOUT" | "STDERR"
no-tty    := "NO-TTY"   ws ("STDOUT" | "STDERR")
args      := "ARGS"     ws string
string    := '"' (literal-char | escape)* '"'
escape    := "\" ("n" | "t" | "r" | "\" | '"')
name      := "SIG" [A-Z]+
```

An unknown escape, directive, or signal name is a parse error. Use block style comments for non-directives.

### Example

```c
#include <stdio.h>

int main(void) {
    int deadbeef = 0xdeadbeef;
    // WAIT-FOR "0xdeadbeef"
    printf("0x%x\n", deadbeef);
    return 0;
}
```

### Fixtures

A test that needs files on disk checks them in under `examples/fixtures/<stem>/`. The runner copies the fixture directory to a fresh scratch directory per run and uses it as the working directory.

