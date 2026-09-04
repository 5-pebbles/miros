# Commit Conventions

Rules for commit message shape in this repository.

## Structure

```
type(scope): description;

[body]

[footers]
```

## Types

| type | when |
| --- | --- |
| `init` | new repository |
| `fix` | bug fix |
| `feat` | new feature |
| `rm` | removes a feature |
| `refactor` | structural change, no behavior change |
| `revert` | undoes a commit |
| `build` | build system or dependencies |
| `docs` | documentation only, never mixed with code |
| `ci/cd` | pipeline config |
| `merge` | merge commit, scope is the source branch |

## Scope

Required. Under 30 characters. The area of the codebase, never an issue or PR number. Examples seen here: `libc`, `examples`, `xtask`, `demo`, `check`, `release`, `readme`, `auxv`, `objects`, `tls`, `allocator`.

## Description

Follows `: ` directly. Lowercase first letter. Imperative present: "add", not "added" or "adds". Ends with `;`. Prefer `&` or `+` over "and". The whole title line stays under 72 characters. The limit applies to the title only. Verify the count:

```sh
echo -n 'type(scope): description;' | wc -c
```

## Body

Usually absent. Include only when the title can't carry it, then keep it to one sentence stating what the diff can't show. Never restate the diff. Never hard-wrap. One paragraph is one line.

## Footers

Format is `TITLE: content;` with the title in caps and the content ending in `;`.

A breaking change gets `!` after the scope, plus a `BREAKING CHANGE: <what broke>;` footer.

A revert uses type `revert`, plus a `REVERT: <sha>;` footer. Separate several SHAs with `&`.

## Examples from this repo's log

```
feat(libc): add dlsym, fstat & fcntl64;
fix(xtask): end the failure excerpt only after test output;
docs(examples): spec the e2e test directive language;
ci/cd(check): run e2e tests via xtask test;
build(demo): forward features arg into build call;
```
