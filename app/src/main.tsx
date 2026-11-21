import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { NotificationProvider } from "./components/Notifications";
import "./styles.css";

// No browser right-click menu (Back, Forward, Reload, Inspect Element). Our own menus open from
// their own handlers; text boxes keep the normal Cut/Copy/Paste menu.
window.addEventListener("contextmenu", (e) => {
  const target = e.target as HTMLElement | null;
  if (target?.closest?.("input, textarea")) return;
  e.preventDefault();
});

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <NotificationProvider>
      <App />
    </NotificationProvider>
  </StrictMode>,
);
