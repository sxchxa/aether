// Signaler un problème : message envoyé directement à l'équipe (adresse jamais affichée),
// ou, à défaut de service configuré, signalement pré-rempli sur GitHub.
import { useEffect, useState } from "react";
import { Bug, Send } from "lucide-react";
import { Button, Card, CardHeader, Toggle } from "../../components/ui";
import { useI18n } from "../../i18n";
import { api, errorMessage } from "../../lib/ipc";
import { useApp } from "../../store/app";

const ISSUES = "https://github.com/sxchxa/aether/issues/new";

export function FeedbackCard() {
  const { m } = useI18n();
  const toast = useApp((s) => s.toast);
  const [available, setAvailable] = useState(false);
  const [message, setMessage] = useState("");
  const [replyTo, setReplyTo] = useState("");
  const [withInfo, setWithInfo] = useState(true);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void api
      .feedbackAvailable()
      .then(setAvailable)
      .catch(() => setAvailable(false));
  }, []);

  const send = async () => {
    setBusy(true);
    try {
      if (available) {
        await api.feedbackSend(message, withInfo, replyTo.trim() || null);
        toast("success", m.feedback.sent);
        setMessage("");
      } else {
        const url = `${ISSUES}?title=${encodeURIComponent(message.slice(0, 80))}&body=${encodeURIComponent(message)}`;
        await api.openLink(url);
      }
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card id="set-feedback">
      <CardHeader icon={<Bug className="size-[18px]" />} tone="rose" title={m.feedback.title} description={m.feedback.desc} />
      <div className="space-y-3 border-t border-white/[0.05] px-6 py-5">
        <textarea
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          maxLength={5000}
          rows={5}
          placeholder={m.feedback.placeholder}
          aria-label={m.feedback.title}
          className="surface-2 w-full resize-y rounded-[10px] px-4 py-3 text-[13.5px] text-white outline-none placeholder:text-[var(--text-3)] focus:border-[var(--accent)]"
        />
        {available && (
          <>
            <input
              type="email"
              value={replyTo}
              onChange={(e) => setReplyTo(e.target.value)}
              placeholder={m.feedback.replyTo}
              aria-label={m.feedback.replyTo}
              className="surface-2 w-full rounded-[10px] px-4 py-2.5 text-[13px] text-white outline-none placeholder:text-[var(--text-3)] focus:border-[var(--accent)]"
            />
            <label className="flex items-center justify-between gap-4 text-[13px] text-[var(--text-2)]">
              <span className="min-w-0">{m.feedback.withInfo}</span>
              <Toggle checked={withInfo} onChange={setWithInfo} label={m.feedback.withInfo} />
            </label>
          </>
        )}
        <div className="flex flex-wrap items-center justify-between gap-3">
          <span className="min-w-0 flex-1 text-[12px] text-[var(--text-3)]">{available ? m.feedback.privacy : m.feedback.viaGithub}</span>
          <Button
            variant="primary"
            size="sm"
            icon={<Send className="size-3.5" />}
            loading={busy}
            disabled={message.trim().length < 5}
            onClick={() => void send()}
          >
            {m.feedback.send}
          </Button>
        </div>
      </div>
    </Card>
  );
}
