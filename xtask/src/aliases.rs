use std::{collections::HashSet, fs, path::PathBuf};

use super::build::workspace_root;

/// Regenerates `aliases.rs` and `aliases.ver` from `linked_aliases.def` on every build, returning the version script path.
pub fn generate() -> PathBuf {
    let root = workspace_root();
    let definition_path = root.join("linked_aliases.def");
    let definitions = fs::read_to_string(&definition_path).expect("read linked_aliases.def");

    // One line per alias set: `target: alias, alias(weak), ...`.
    let mut assembly = String::new();
    let mut exports = String::new();
    let mut seen = HashSet::new();

    for (line_number, raw_line) in definitions.lines().enumerate() {
        let line = raw_line.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }

        let (target, aliases) = line.split_once(':').unwrap_or_else(|| {
            panic!(
                "linked_aliases.def:{}: expected `target: alias, ...`",
                line_number + 1
            )
        });
        let target = target.trim();

        for alias in aliases
            .split(',')
            .map(str::trim)
            .filter(|alias| !alias.is_empty())
        {
            let (name, weak) = match alias.strip_suffix("(weak)") {
                Some(name) => (name.trim(), true),
                None => (alias, false),
            };

            // .set silently redefines, so a duplicated alias would otherwise retarget without a word.
            assert!(
                seen.insert(name),
                "linked_aliases.def:{}: duplicate alias `{name}`",
                line_number + 1
            );

            assembly.push_str(&format!(
                "{visibility} {name}\n.set {name}, {target}\n",
                visibility = if weak { ".weak" } else { ".globl" },
            ));
            // rustc's generated version script localizes symbols it doesn't know about.
            // An alias emitted purely in asm would never reach .dynsym without this script.
            exports.push_str(&format!("    {name};\n"));
        }
    }

    let generated = root.join("target/generated");
    fs::create_dir_all(&generated).unwrap();

    // global_asm! requires at least one template string, so an empty def falls back to `""`.
    let templates = assembly
        .lines()
        .map(|line| format!("    {line:?},\n"))
        .collect::<String>();
    let templates = Some(templates)
        .filter(|templates| !templates.is_empty())
        .unwrap_or_else(|| "    \"\",\n".to_string());

    let rust_source = format!("core::arch::global_asm!(\n{templates});");
    fs::write(generated.join("aliases.rs"), rust_source).unwrap();

    let version_script = generated.join("aliases.ver");
    fs::write(
        &version_script,
        format!("{{\n  global:\n{exports}  local: *;\n}};\n"),
    )
    .unwrap();

    version_script
}
