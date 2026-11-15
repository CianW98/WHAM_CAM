import { useEffect, useState } from "react";
import { getDisplayData, getDocument, type DisplayData, type Document } from "./engine";
import { useNotify } from "./components/Notifications";
import Toolbar from "./components/Toolbar";
import Tree from "./components/Tree";
import Properties from "./components/Properties";
import Viewport from "./components/Viewport";
import { selectedObjectId, type Selection } from "./selection";

export default function App() {
  const notify = useNotify();
  const [doc, setDoc] = useState<Document | null>(null);
  const [display, setDisplay] = useState<DisplayData | null>(null);
  const [selection, setSelection] = useState<Selection>(null);

  useEffect(() => {
    Promise.all([getDocument(), getDisplayData()])
      .then(([d, dd]) => {
        setDoc(d);
        setDisplay(dd);
      })
      .catch((err) => notify("error", `Could not reach the CAM engine: ${String(err)}`));
  }, [notify]);

  return (
    <div className="app">
      <Toolbar onAction={(name) => notify("info", `${name} is not built yet.`)} />
      <div className="side">
        <section className="panel tree-panel">
          <h2>Project</h2>
          <Tree doc={doc} selection={selection} onSelect={setSelection} />
        </section>
        <section className="panel props-panel">
          <h2>Properties</h2>
          <Properties doc={doc} selection={selection} />
        </section>
      </div>
      <Viewport display={display} selectedId={selectedObjectId(selection)} />
    </div>
  );
}
