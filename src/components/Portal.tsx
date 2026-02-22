import { createPortal } from "react-dom";

export function Portal({ children }: { children: React.ReactNode }) {
  return createPortal(children, document.body);
}
