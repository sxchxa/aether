// Superpositions : notifications (toasts), modale de confirmation, tiroir du journal.
import { useEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { CircleAlert, CircleCheck, Info, ScrollText, Trash2, TriangleAlert, X } from "lucide-react";
import { useI18n } from "../i18n";
import { api } from "../lib/ipc";
import { formatTime } from "../lib/format";
import type { LogLevel } from "../lib/types";
import { useApp, type ToastKind } from "../store/app";
import { Button, cx } from "./ui";

const TOAST_ICON: Record<ToastKind, ReactNode> = {
  success: <CircleCheck className="size-4 text-emerald" />,
  info: <Info className="size-4 text-cyan" />,
  warn: <TriangleAlert className="size-4 text-amber" />,
  error: <CircleAlert className="size-4 text-rose" />,
};

export function Toasts() {
  const toasts = useApp((s) => s.toasts);
  const dismiss = useApp((s) => s.dismissToast);
  return (
    <div className="pointer-events-none absolute right-5 bottom-5 z-50 flex w-[360px] flex-col gap-2" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className="glass-float pointer-events-auto flex items-start gap-3 rounded-xl px-4 py-3 animate-slide-up">
          <span className="mt-0.5">{TOAST_ICON[t.kind]}</span>
          <p className="flex-1 text-[13px] leading-snug text-slate-100">{t.text}</p>
          <button type="button" onClick={() => dismiss(t.id)} className="text-slate-500 hover:text-white" aria-label="×">
            <X className="size-3.5" />
          </button>
        </div>
      ))}
    </div>
  );
}

interface ModalProps {
  open: boolean;
  title: string;
  children: ReactNode;
  onClose: () => void;
  footer?: ReactNode;
}

export function Modal({ open, title, children, onClose, footer }: ModalProps) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);
  if (!open) return null;
  // Rendue à la racine du document : toujours centrée sur l'écran, quelle que soit la position
  // de défilement de la page qui l'ouvre.
  return createPortal(
    <div className="fixed inset-0 z-[100] grid place-items-center bg-black/65 animate-fade-in" onMouseDown={onClose}>
      <div
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className="glass-float w-[460px] max-w-[92%] rounded-2xl animate-scale-in"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="border-b border-white/[0.06] px-5 py-4 text-[15px] font-semibold text-white">{title}</div>
        <div className="px-5 py-4 text-[13px] leading-relaxed text-[var(--text-2)]">{children}</div>
        {footer && <div className="flex justify-end gap-2 border-t border-white/[0.06] px-5 py-3.5">{footer}</div>}
      </div>
    </div>,
    document.body,
  );
}

interface ConfirmProps {
  open: boolean;
  title: string;
  body: ReactNode;
  confirmLabel: string;
  danger?: boolean;
  onConfirm: () => void;
  onClose: () => void;
}

export function ConfirmDialog({ open, title, body, confirmLabel, danger, onConfirm, onClose }: ConfirmProps) {
  const { m } = useI18n();
  return (
    <Modal
      open={open}
      title={title}
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {m.common.cancel}
          </Button>
          <Button
            variant={danger ? "danger" : "primary"}
            onClick={() => {
              onConfirm();
              onClose();
            }}
          >
            {confirmLabel}
          </Button>
        </>
      }
    >
      {body}
    </Modal>
  );
}

const LEVEL_STYLE: Record<LogLevel, string> = {
  info: "text-slate-400",
  success: "text-emerald-300",
  skip: "text-cyan",
  preview: "text-indigo-soft",
  warn: "text-amber-300",
  error: "text-rose-300",
};

export function LogDrawer() {
  const { m, lang } = useI18n();
  const open = useApp((s) => s.logsOpen);
  const setOpen = useApp((s) => s.setLogsOpen);
  const logs = useApp((s) => s.logs);
  const setLogs = useApp((s) => s.setLogs);
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (open) endRef.current?.scrollIntoView({ block: "end" });
  }, [open, logs.length]);

  // Panneau flottant : entièrement masqué quand il est fermé (ouvert depuis l’icône « Journal »
  // en haut à droite ou la palette Ctrl+K), comme le reste des commandes de la fenêtre.
  return (
    <div
      aria-hidden={!open}
      className={cx(
        "glass-float absolute right-4 bottom-4 left-4 z-30 flex h-[300px] flex-col rounded-[16px] transition-[transform,opacity] duration-300 ease-[cubic-bezier(0.2,0,0,1)] will-change-transform",
        open ? "translate-y-0 opacity-100" : "pointer-events-none translate-y-6 opacity-0",
      )}
    >
      <div className="flex h-12 shrink-0 items-center gap-2 border-b border-white/[0.06] px-4">
        <ScrollText className="size-4 text-[var(--steel)]" aria-hidden />
        <span className="text-[13px] text-zinc-100">{m.logs.title}</span>
        <span className="tabular rounded-full bg-white/[0.06] px-1.5 text-[11px] text-zinc-500">{logs.length}</span>
        <span className="flex-1" />
        <Button
          size="sm"
          variant="ghost"
          icon={<Trash2 className="size-3.5" />}
          onClick={() => void api.logsClear().then(() => setLogs([]))}
        >
          {m.logs.clear}
        </Button>
        <button
          type="button"
          onClick={() => setOpen(false)}
          aria-label={m.common.close}
          className="grid size-8 place-items-center rounded-[9px] text-zinc-400 transition-colors hover:bg-white/[0.06] hover:text-zinc-100"
        >
          <X className="size-4" aria-hidden />
        </button>
      </div>
      {open && (
        <div className="scroll-area selectable min-h-0 flex-1 px-4 py-3 font-mono text-[11.5px] leading-[1.7]">
          {logs.length === 0 && <div className="text-zinc-500">{m.logs.empty}</div>}
          {logs.map((l, i) => (
            <div key={`${l.ts}-${i}`} className="flex gap-3">
              <span className="tabular shrink-0 text-zinc-600">{formatTime(l.ts, lang)}</span>
              <span className="w-20 shrink-0 truncate text-zinc-500">{l.module}</span>
              <span className={LEVEL_STYLE[l.level]}>{l.message}</span>
            </div>
          ))}
          <div ref={endRef} />
        </div>
      )}
    </div>
  );
}
