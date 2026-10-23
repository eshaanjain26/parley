import { CheckCircle2 } from "lucide-react";
import { ReactNode, createContext, useCallback, useContext, useRef, useState } from "react";

const SHOW_MS = 2800;

interface Toast {
  id: number;
  message: string;
}

const Ctx = createContext<(message: string) => void>(() => {});

/** Shows a short confirmation ("Moved to Acme") at the bottom of the window. */
export const useToast = () => useContext(Ctx);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const next = useRef(1);

  const show = useCallback((message: string) => {
    const id = next.current++;
    // Only the latest few stay visible.
    setToasts((t) => [...t.slice(-2), { id, message }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), SHOW_MS);
  }, []);

  return (
    <Ctx.Provider value={show}>
      {children}
      <div className="toast-stack" role="status" aria-live="polite">
        {toasts.map((t) => (
          <div key={t.id} className="toast-item">
            <CheckCircle2 size={16} aria-hidden />
            {t.message}
          </div>
        ))}
      </div>
    </Ctx.Provider>
  );
}
