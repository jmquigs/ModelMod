# ModelMod - Claude Code Notes

## Rust Build

This is a Windows-only project, but cross-compilation works from Linux.

- Workspace root: `Native/`
- To check Rust code compiles: `cd Native && cargo check --target x86_64-pc-windows-msvc`
- Always use `--target x86_64-pc-windows-msvc` — without it, cargo will target Linux and fail
- Runtime tests (`cargo test`) won't work on Linux since they call Windows APIs, but compilation checks are valid
