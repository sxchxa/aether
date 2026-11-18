// Section « Réseau & Latence ».
import { useEffect, useState } from "react";
import { Cable, Gauge, RefreshCw, RotateCcw, Rocket, Trophy, Wifi, Wrench } from "lucide-react";
import { TweakList } from "../../components/TweakCard";
import { SpeedTestCard } from "./SpeedTestCard";
import { Button, Card, CardHeader, cx, PageHeader } from "../../components/ui";
import { useI18n } from "../../i18n";
import { api, errorMessage } from "../../lib/ipc";
import type { DnsBenchResult, DnsConfig, NicSnapshot } from "../../lib/types";
import { useApp } from "../../store/app";

export const NETWORK_TWEAKS = {
  nic: ["nic.powerSaving", "nic.wakeMagic", "nic.wakePattern", "nic.eee", "nic.interruptModeration"],
  security: ["net.doh", "net.llmnr", "net.smb1"],
  tcp: ["tcp.throttling"],
  bandwidth: ["net.deliveryOptimization"],
};

export function NetworkPage() {
  const { m, lang } = useI18n();
  const ensureTweaks = useApp((s) => s.ensureTweaks);
  const refreshHistory = useApp((s) => s.refreshHistory);
  const toast = useApp((s) => s.toast);
  const inspect = useApp((s) => s.inspect);
  const [nic, setNic] = useState<NicSnapshot | null>(null);
  const [dns, setDns] = useState<DnsConfig | null>(null);
  const [bench, setBench] = useState<DnsBenchResult[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const loadDns = () =>
    void api
      .dnsCurrent()
      .then(setDns)
      .catch(() => setDns(null));

  useEffect(() => {
    void api
      .networkAdapter()
      .then(setNic)
      .catch(() => setNic({ adapter: null, properties: [] }));
    void ensureTweaks([...NETWORK_TWEAKS.nic, ...NETWORK_TWEAKS.tcp, ...NETWORK_TWEAKS.bandwidth, ...NETWORK_TWEAKS.security]);
    loadDns();
  }, [ensureTweaks]);

  const run = async (key: string, fn: () => Promise<void>) => {
    setBusy(key);
    try {
      await fn();
    } catch (e) {
      toast("error", errorMessage(e));
    } finally {
      setBusy(null);
    }
  };

  const applyDns = (r: DnsBenchResult) =>
    run(`dns-${r.id}`, async () => {
      const changed = await api.dnsApply(r.primary, r.secondary);
      toast(changed ? "success" : "info", changed ? `DNS ${r.name}` : m.toast.previewOnly);
      loadDns();
      await refreshHistory();
    });

  const reachable = (bench ?? []).filter((r) => r.avgMs !== null);
  const best = reachable[0];
  const worst = Math.max(1, ...reachable.map((r) => r.avgMs ?? 0));
  const nf = new Intl.NumberFormat(lang, { maximumFractionDigits: 1 });
  const AdapterIcon = nic?.adapter?.media.includes("802.11") ? Wifi : Cable;

  return (
    <div className="space-y-10">
      <PageHeader section="network" title={m.network.title} subtitle={m.network.subtitle} />

      <div className="grid grid-cols-1 items-start gap-10">
        <Card id="nw-nic" className="">
          <CardHeader
            icon={<AdapterIcon className="size-[18px]" />}
            tone="emerald"
            title={m.network.nic}
            description={
              nic?.adapter
                ? `${nic.adapter.name} · ${nic.adapter.description} · ${nic.adapter.linkSpeed}`
                : nic
                  ? m.network.nicNone
                  : m.common.loading
            }
          />
          {nic?.adapter && <TweakList ids={NETWORK_TWEAKS.nic} />}
          <div className="px-6 pb-4 text-[11.5px] text-[var(--text-3)]">{m.network.nicReset}</div>
        </Card>

        <div className="space-y-10">
          <Card id="nw-tcp" className="">
            <CardHeader icon={<Gauge className="size-[18px]" />} tone="emerald" title={m.network.tcp} />
            <TweakList ids={NETWORK_TWEAKS.tcp} />
            <div className="px-6 pb-2 text-[11px] font-semibold uppercase tracking-[0.12em] text-[var(--text-3)]">
              {m.network.bandwidth}
            </div>
            <TweakList ids={NETWORK_TWEAKS.bandwidth} />
          </Card>

          <Card id="nw-security" className="">
            <CardHeader title={m.network.security} />
            <TweakList ids={NETWORK_TWEAKS.security} />
          </Card>

          <Card id="nw-maintenance" className="">
            <CardHeader
              icon={<Wrench className="size-[18px]" />}
              tone="slate"
              title={m.network.maintenance}
              onHelp={() => inspect({ kind: "action", id: "winsock", technical: "ipconfig /flushdns\nnetsh winsock reset" })}
            />
            <div className="flex flex-wrap gap-2 px-5 pb-5">
              <Button
                icon={<RefreshCw className="size-4" />}
                loading={busy === "flush"}
                onClick={() =>
                  void run("flush", async () => {
                    await api.flushDns();
                    toast("success", m.network.flushDone);
                  })
                }
              >
                {m.network.flushDns}
              </Button>
              <Button
                icon={<RotateCcw className="size-4" />}
                loading={busy === "winsock"}
                onClick={() =>
                  void run("winsock", async () => {
                    const r = await api.winsockReset();
                    toast(r.executed ? "warn" : "info", r.executed ? m.network.winsockDone : m.toast.previewOnly);
                  })
                }
              >
                {m.network.winsock}
              </Button>
            </div>
          </Card>
        </div>

        <Card id="nw-dns" className="">
          <CardHeader
            icon={<Rocket className="size-[18px]" />}
            tone="sky"
            title={m.network.dns}
            description={
              dns
                ? `${m.network.dnsCurrent} : ${dns.dhcp ? `${m.network.dnsDhcp} — ` : ""}${dns.servers.join(", ") || "—"}`
                : m.common.loading
            }
            onHelp={() =>
              inspect({
                kind: "action",
                id: "dns",
                technical:
                  "UDP/53 × 5 (A) → Set-DnsClientServerAddress -InterfaceIndex <n> -ServerAddresses …\nSet-DnsClientServerAddress -ResetServerAddresses",
              })
            }
            actions={
              <>
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={!dns || dns.dhcp}
                  loading={busy === "dhcp"}
                  onClick={() =>
                    void run("dhcp", async () => {
                      const changed = await api.dnsReset();
                      toast(changed ? "success" : "info", changed ? m.network.dnsReset : m.toast.previewOnly);
                      loadDns();
                      await refreshHistory();
                    })
                  }
                >
                  {m.network.dnsReset}
                </Button>
                <Button
                  size="sm"
                  loading={busy === "bench"}
                  onClick={() => void run("bench", async () => setBench(await api.dnsBenchmark()))}
                >
                  {busy === "bench" ? m.network.dnsRunning : m.network.dnsRun}
                </Button>
                {best && (
                  <Button
                    size="sm"
                    variant="primary"
                    icon={<Trophy className="size-3.5" />}
                    loading={busy === `dns-${best.id}`}
                    onClick={() => void applyDns(best)}
                  >
                    {m.network.dnsApplyBest}
                  </Button>
                )}
              </>
            }
          />
          {bench && (
            <div className="space-y-1.5 px-5 pb-5">
              {bench.map((r) => {
                const isCurrent = dns?.servers[0] === r.primary;
                return (
                  <div key={r.id} className="surface-2 flex items-center gap-4 rounded-[10px] px-4 py-3">
                    <div className="w-36 shrink-0">
                      <div className="flex items-center gap-1.5 text-[13px] font-medium text-white">
                        {r.name}
                        {r === best && <Trophy className="size-3.5 text-amber" aria-label={m.network.dnsFastest} />}
                      </div>
                      <div className="font-mono text-[11px] text-[var(--text-3)]">{r.primary}</div>
                    </div>
                    <div className="h-2 flex-1 overflow-hidden rounded-full bg-[var(--track)]">
                      {r.avgMs !== null && (
                        <div
                          className={cx("meter-fill h-full rounded-full", r === best ? "bg-emerald" : "bg-indigo")}
                          style={{ transform: `scaleX(${Math.max(4, (r.avgMs / worst) * 100) / 100})` }}
                        />
                      )}
                    </div>
                    <div className="tabular w-24 text-right text-[13px] font-semibold text-white">
                      {r.avgMs !== null ? (
                        `${nf.format(r.avgMs)} ms`
                      ) : (
                        <span className="font-normal text-rose-300">{m.network.dnsFailed}</span>
                      )}
                    </div>
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={r.avgMs === null || isCurrent}
                      loading={busy === `dns-${r.id}`}
                      onClick={() => void applyDns(r)}
                    >
                      {m.network.dnsApply}
                    </Button>
                  </div>
                );
              })}
            </div>
          )}
        </Card>

        <SpeedTestCard />
      </div>
    </div>
  );
}
