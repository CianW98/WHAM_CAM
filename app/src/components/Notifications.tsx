import { createContext, useCallback, useContext, useState, type ReactNode } from "react";

type Kind = "info" | "warning" | "error";
interface Note {
  id: number;
  kind: Kind;
  text: string;
}
type Notify = (kind: Kind, text: string) => void;

const NotifyContext = createContext<Notify>(() => {});

/** Show a pop-up message. Info messages fade out; warnings and errors stay until closed. */
export function useNotify(): Notify {
  return useContext(NotifyContext);
}

const INFO_TIMEOUT_MS = 4000;
let nextId = 1;

export function NotificationProvider({ children }: { children: ReactNode }) {
  const [notes, setNotes] = useState<Note[]>([]);

  const dismiss = useCallback((id: number) => {
    setNotes((ns) => ns.filter((n) => n.id !== id));
  }, []);

  const notify = useCallback<Notify>(
    (kind, text) => {
      const id = nextId++;
      setNotes((ns) => [...ns, { id, kind, text }]);
      if (kind === "info") setTimeout(() => dismiss(id), INFO_TIMEOUT_MS);
    },
    [dismiss],
  );

  return (
    <NotifyContext.Provider value={notify}>
      {children}
      <div className="notifications">
        {notes.map((n) => (
          <div key={n.id} className={`note note-${n.kind}`} role={n.kind === "info" ? "status" : "alert"}>
            <span>{n.text}</span>
            <button className="note-close" onClick={() => dismiss(n.id)} aria-label="Dismiss">
              ×
            </button>
          </div>
        ))}
      </div>
    </NotifyContext.Provider>
  );
}
