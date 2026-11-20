/**
 * What is selected. Drawing objects can be multi-selected (tree or drawing area);
 * the project, a layer, a part, an operation or a tool library is a single tree selection shown in the properties
 * panel.
 */
export type Selection =
  | { kind: "objects"; ids: number[] }
  | { kind: "project" }
  | { kind: "layer"; layer: number }
  | { kind: "part"; part: number }
  /** The Machining heading in the tree. */
  | { kind: "machining" }
  | { kind: "operation"; id: number }
  /** A tool library (Setup tab), by name. */
  | { kind: "library"; name: string }
  /** A machine (Setup tab), by name. */
  | { kind: "machine"; name: string };

export const NOTHING: Selection = { kind: "objects", ids: [] };

export function selectedIds(sel: Selection): number[] {
  return sel.kind === "objects" ? sel.ids : [];
}

/** Select `ids`, either replacing the selection or (with `add`) toggling them in/out of it. */
export function selectObjects(sel: Selection, ids: number[], add: boolean): Selection {
  if (!add) return { kind: "objects", ids };
  const current = new Set(selectedIds(sel));
  for (const id of ids) {
    if (current.has(id)) current.delete(id);
    else current.add(id);
  }
  return { kind: "objects", ids: [...current] };
}
