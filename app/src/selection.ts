/** What is currently selected in the tree / drawing. */
export type Selection =
  | { kind: "project" }
  | { kind: "layer"; layer: number }
  | { kind: "object"; layer: number; id: number }
  | { kind: "part"; part: number }
  | null;

export function selectedObjectId(sel: Selection): number | null {
  return sel?.kind === "object" ? sel.id : null;
}
