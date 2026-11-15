# WHAM_CAM roadmap (after v1)

Ideas for after v1, with the reasoning and any decisions made so far. Nothing here is being built
yet: v1 (see `docs/spec.md`) comes first. The order within each group is rough and can change.

## Soon after v1

- **Machine-specific post-processors**: RepRapFirmware/WorkBee, Grbl, LinuxCNC, Mach3, each with its
  own settings panel per machine type. v1 has one generic post-processor only.
- **Inches**, together with CAM styles/templates and workspaces. v1 is mm only.
- **Lead-ins / lead-outs.** v1 uses a straight plunge at the plunge feed rate.
- **Holding tabs.**

## Later

- **Browser version.** The same interface, with the engine compiled to WebAssembly and served as
  static files. All CAM work runs on the user's own computer; nothing is processed on a server.
  Open a web address and it runs, no install.
  - Hosting (discussed 2026-11-15, decide when this is built): the repo is public (Apache 2.0), so
    **GitHub Pages** hosts it for free and is the simplest option. **Cloudflare** (Workers static
    hosting, also free) is the better choice if we want WebAssembly to use several processor cores
    (this needs server settings GitHub Pages can't set) or small online features later, such as an
    update check or sharing a project by link.
- **Predefined styles in the library** for common kinds of CNC work, e.g. PCB cutting, V-carving
  and other common uses. List to be filled in.

## Maybe / needs thought

These may clash with the "keep it simple" principle. Ask before pursuing.

- **3D operations.**

## Decided against

- Importing other CAM programs' project files: never wanted.
- Machine control: far out of scope.
