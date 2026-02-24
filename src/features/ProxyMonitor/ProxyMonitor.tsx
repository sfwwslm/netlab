import React, { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { StyledButton } from "@/components/styled/StyledButton";
import {
  ActionRow,
  ClientsBody,
  ClientsCell,
  ClientsHead,
  ClientsHeader,
  ClientsPanel,
  ClientsRow,
  ClientsTable,
  ClientsTitle,
  ConfigPanel,
  ConfigRow,
  ConfigSection,
  ContentPanel,
  EmptyState,
  Label,
  ProxyContainer,
  SectionTitle,
  StatCard,
  StatLabel,
  StatValue,
  StatsGrid,
  StatusLine,
  StatusList,
  StatusItem,
  StyledInput,
} from "./ProxyMonitor.styles";

type ProxyClientSnapshot = {
  ip: string;
  activeConnections: number;
  totalConnections: number;
  totalRequests: number;
  bytesIn: number;
  bytesOut: number;
  lastSeenMs: number;
  topTargets: { target: string; count: number }[];
};

type ProxySnapshot = {
  uptimeMs: number;
  activeConnections: number;
  totalConnections: number;
  totalRequests: number;
  bytesIn: number;
  bytesOut: number;
  clients: ProxyClientSnapshot[];
};

const formatNumber = (value: number, fraction = 1) =>
  Number.isFinite(value) ? value.toFixed(fraction) : "0";

const formatBytes = (value: number) => {
  if (!Number.isFinite(value)) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let current = Math.max(0, value);
  let idx = 0;
  while (current >= 1024 && idx < units.length - 1) {
    current /= 1024;
    idx += 1;
  }
  const fraction = current >= 10 ? 0 : 1;
  return `${formatNumber(current, fraction)} ${units[idx]}`;
};

const formatUptime = (ms: number) => {
  if (!Number.isFinite(ms) || ms <= 0) return "-";
  const secs = Math.floor(ms / 1000);
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${(secs / 60).toFixed(1)}m`;
  return `${(secs / 3600).toFixed(1)}h`;
};

const formatAge = (lastSeenMs: number) => {
  if (!Number.isFinite(lastSeenMs) || lastSeenMs === 0) return "-";
  const age = Math.max(0, Date.now() - lastSeenMs);
  if (age < 1000) return `${Math.round(age)} ms`;
  return `${(age / 1000).toFixed(1)} s`;
};

const formatTopTargets = (targets: { target: string; count: number }[]) => {
  if (!targets || targets.length === 0) return "-";
  return targets
    .slice(0, 3)
    .map((entry) => `${entry.target} x${entry.count}`)
    .join(", ");
};

const ProxyMonitor: React.FC = () => {
  const { t } = useTranslation();
  const [listenHost, setListenHost] = useState("0.0.0.0");
  const [listenPort, setListenPort] = useState(8899);
  const [reportIntervalMs, setReportIntervalMs] = useState(500);
  const [running, setRunning] = useState(false);
  const [status, setStatus] = useState(t("tools.proxy.statusIdle"));
  const [statusHistory, setStatusHistory] = useState<string[]>([]);
  const [snapshot, setSnapshot] = useState<ProxySnapshot | null>(null);

  const [socksHost, setSocksHost] = useState("0.0.0.0");
  const [socksPort, setSocksPort] = useState(1080);
  const [socksReportIntervalMs, setSocksReportIntervalMs] = useState(500);
  const [socksEnableUdp, setSocksEnableUdp] = useState(true);
  const [socksRunning, setSocksRunning] = useState(false);
  const [socksStatus, setSocksStatus] = useState(
    t("tools.proxy.socksStatusIdle"),
  );
  const [socksStatusHistory, setSocksStatusHistory] = useState<string[]>([]);
  const [socksSnapshot, setSocksSnapshot] = useState<ProxySnapshot | null>(
    null,
  );

  useEffect(() => {
    const unlistenStatus = listen<string>("proxy:status", (event) => {
      setStatus(event.payload);
      setStatusHistory((prev) => {
        const next = [event.payload, ...prev];
        return next.slice(0, 5);
      });
    });
    const unlistenSnapshot = listen<ProxySnapshot>(
      "proxy:snapshot",
      (event) => {
        setSnapshot(event.payload);
      },
    );
    const unlistenSocksStatus = listen<string>("socks5:status", (event) => {
      setSocksStatus(event.payload);
      setSocksStatusHistory((prev) => {
        const next = [event.payload, ...prev];
        return next.slice(0, 5);
      });
    });
    const unlistenSocksSnapshot = listen<ProxySnapshot>(
      "socks5:snapshot",
      (event) => {
        setSocksSnapshot(event.payload);
      },
    );
    return () => {
      unlistenStatus.then((fn) => fn());
      unlistenSnapshot.then((fn) => fn());
      unlistenSocksStatus.then((fn) => fn());
      unlistenSocksSnapshot.then((fn) => fn());
    };
  }, []);

  const handleStart = useCallback(async () => {
    await invoke("start_proxy", {
      config: {
        listenHost,
        listenPort: Number(listenPort),
        reportIntervalMs: Number(reportIntervalMs),
      },
    });
    setRunning(true);
  }, [listenHost, listenPort, reportIntervalMs]);

  const handleStop = useCallback(async () => {
    await invoke("stop_proxy");
    setRunning(false);
    setStatus(t("tools.proxy.statusStopped"));
    setStatusHistory((prev) => {
      const next = [t("tools.proxy.statusStopped"), ...prev];
      return next.slice(0, 5);
    });
  }, [t]);

  const handleStartSocks = useCallback(async () => {
    await invoke("start_socks5", {
      config: {
        listenHost: socksHost,
        listenPort: Number(socksPort),
        reportIntervalMs: Number(socksReportIntervalMs),
        enableUdp: socksEnableUdp,
      },
    });
    setSocksRunning(true);
  }, [socksHost, socksPort, socksReportIntervalMs, socksEnableUdp]);

  const handleStopSocks = useCallback(async () => {
    await invoke("stop_socks5");
    setSocksRunning(false);
    setSocksStatus(t("tools.proxy.socksStatusStopped"));
    setSocksStatusHistory((prev) => {
      const next = [t("tools.proxy.socksStatusStopped"), ...prev];
      return next.slice(0, 5);
    });
  }, [t]);

  const stats = useMemo(() => {
    return {
      uptime: snapshot?.uptimeMs ?? 0,
      activeConnections: snapshot?.activeConnections ?? 0,
      totalConnections: snapshot?.totalConnections ?? 0,
      totalRequests: snapshot?.totalRequests ?? 0,
      bytesIn: snapshot?.bytesIn ?? 0,
      bytesOut: snapshot?.bytesOut ?? 0,
      clients: snapshot?.clients ?? [],
    };
  }, [snapshot]);

  const socksStats = useMemo(() => {
    return {
      uptime: socksSnapshot?.uptimeMs ?? 0,
      activeConnections: socksSnapshot?.activeConnections ?? 0,
      totalConnections: socksSnapshot?.totalConnections ?? 0,
      totalRequests: socksSnapshot?.totalRequests ?? 0,
      bytesIn: socksSnapshot?.bytesIn ?? 0,
      bytesOut: socksSnapshot?.bytesOut ?? 0,
      clients: socksSnapshot?.clients ?? [],
    };
  }, [socksSnapshot]);

  return (
    <ProxyContainer>
      <ConfigPanel>
        <ConfigSection>
          <SectionTitle>{t("tools.proxy.configTitle")}</SectionTitle>
          <ConfigRow>
            <Label>{t("tools.proxy.listenHost")}</Label>
            <StyledInput
              value={listenHost}
              onChange={(event) => setListenHost(event.target.value)}
              placeholder="0.0.0.0"
            />
          </ConfigRow>
          <ConfigRow>
            <Label>{t("tools.proxy.listenPort")}</Label>
            <StyledInput
              type="number"
              value={listenPort}
              onChange={(event) => setListenPort(Number(event.target.value))}
            />
          </ConfigRow>
          <ConfigRow>
            <Label>{t("tools.proxy.reportInterval")}</Label>
            <StyledInput
              type="number"
              value={reportIntervalMs}
              onChange={(event) =>
                setReportIntervalMs(Number(event.target.value))
              }
            />
          </ConfigRow>
        </ConfigSection>
        <ConfigSection>
          <SectionTitle>{t("tools.proxy.controlTitle")}</SectionTitle>
          <ActionRow>
            <StyledButton
              variant={running ? "ghost" : "primary"}
              onClick={handleStart}
              disabled={running}
            >
              {t("tools.proxy.start")}
            </StyledButton>
            <StyledButton
              variant={running ? "danger" : "ghost"}
              onClick={handleStop}
              disabled={!running}
            >
              {t("tools.proxy.stop")}
            </StyledButton>
          </ActionRow>
          <StatusLine>{status}</StatusLine>
          {statusHistory.length > 0 && (
            <StatusList>
              {statusHistory.map((item, index) => (
                <StatusItem key={`${item}-${index}`}>{item}</StatusItem>
              ))}
            </StatusList>
          )}
        </ConfigSection>

        <ConfigSection>
          <SectionTitle>{t("tools.proxy.socksTitle")}</SectionTitle>
          <ConfigRow>
            <Label>{t("tools.proxy.listenHost")}</Label>
            <StyledInput
              value={socksHost}
              onChange={(event) => setSocksHost(event.target.value)}
              placeholder="0.0.0.0"
            />
          </ConfigRow>
          <ConfigRow>
            <Label>{t("tools.proxy.listenPort")}</Label>
            <StyledInput
              type="number"
              value={socksPort}
              onChange={(event) => setSocksPort(Number(event.target.value))}
            />
          </ConfigRow>
          <ConfigRow>
            <Label>{t("tools.proxy.reportInterval")}</Label>
            <StyledInput
              type="number"
              value={socksReportIntervalMs}
              onChange={(event) =>
                setSocksReportIntervalMs(Number(event.target.value))
              }
            />
          </ConfigRow>
          <ActionRow>
            <StyledButton
              variant={socksEnableUdp ? "primary" : "ghost"}
              onClick={() => setSocksEnableUdp((prev) => !prev)}
            >
              {socksEnableUdp
                ? t("tools.proxy.socksUdpOn")
                : t("tools.proxy.socksUdpOff")}
            </StyledButton>
          </ActionRow>
          <ActionRow>
            <StyledButton
              variant={socksRunning ? "ghost" : "primary"}
              onClick={handleStartSocks}
              disabled={socksRunning}
            >
              {t("tools.proxy.socksStart")}
            </StyledButton>
            <StyledButton
              variant={socksRunning ? "danger" : "ghost"}
              onClick={handleStopSocks}
              disabled={!socksRunning}
            >
              {t("tools.proxy.socksStop")}
            </StyledButton>
          </ActionRow>
          <StatusLine>{socksStatus}</StatusLine>
          {socksStatusHistory.length > 0 && (
            <StatusList>
              {socksStatusHistory.map((item, index) => (
                <StatusItem key={`${item}-${index}`}>{item}</StatusItem>
              ))}
            </StatusList>
          )}
        </ConfigSection>
      </ConfigPanel>

      <ContentPanel>
        <SectionTitle>{t("tools.proxy.httpTitle")}</SectionTitle>
        <StatsGrid>
          <StatCard>
            <StatLabel>{t("tools.proxy.uptime")}</StatLabel>
            <StatValue>{formatUptime(stats.uptime)}</StatValue>
          </StatCard>
          <StatCard>
            <StatLabel>{t("tools.proxy.activeConnections")}</StatLabel>
            <StatValue>{stats.activeConnections}</StatValue>
          </StatCard>
          <StatCard>
            <StatLabel>{t("tools.proxy.totalConnections")}</StatLabel>
            <StatValue>{stats.totalConnections}</StatValue>
          </StatCard>
          <StatCard>
            <StatLabel>{t("tools.proxy.totalRequests")}</StatLabel>
            <StatValue>{stats.totalRequests}</StatValue>
          </StatCard>
          <StatCard>
            <StatLabel>{t("tools.proxy.bytesIn")}</StatLabel>
            <StatValue>{formatBytes(stats.bytesIn)}</StatValue>
          </StatCard>
          <StatCard>
            <StatLabel>{t("tools.proxy.bytesOut")}</StatLabel>
            <StatValue>{formatBytes(stats.bytesOut)}</StatValue>
          </StatCard>
        </StatsGrid>

        <ClientsPanel>
          <ClientsHeader>
            <ClientsTitle>{t("tools.proxy.clientsTitle")}</ClientsTitle>
            <span>
              {t("tools.proxy.clientsCount", {
                count: stats.clients.length,
              })}
            </span>
          </ClientsHeader>
          <ClientsBody>
            {stats.clients.length === 0 ? (
              <EmptyState>{t("tools.proxy.noClients")}</EmptyState>
            ) : (
              <ClientsTable>
                <ClientsHead>
                  <tr>
                    <th>{t("tools.proxy.table.ip")}</th>
                    <th>{t("tools.proxy.table.active")}</th>
                    <th>{t("tools.proxy.table.total")}</th>
                    <th>{t("tools.proxy.table.requests")}</th>
                    <th>{t("tools.proxy.table.bytesIn")}</th>
                    <th>{t("tools.proxy.table.bytesOut")}</th>
                    <th>{t("tools.proxy.table.lastSeen")}</th>
                    <th>{t("tools.proxy.table.topTargets")}</th>
                  </tr>
                </ClientsHead>
                <tbody>
                  {stats.clients.slice(0, 50).map((client) => (
                    <ClientsRow key={client.ip}>
                      <ClientsCell>{client.ip}</ClientsCell>
                      <ClientsCell>{client.activeConnections}</ClientsCell>
                      <ClientsCell>{client.totalConnections}</ClientsCell>
                      <ClientsCell>{client.totalRequests}</ClientsCell>
                      <ClientsCell>{formatBytes(client.bytesIn)}</ClientsCell>
                      <ClientsCell>{formatBytes(client.bytesOut)}</ClientsCell>
                      <ClientsCell>{formatAge(client.lastSeenMs)}</ClientsCell>
                      <ClientsCell>
                        {formatTopTargets(client.topTargets)}
                      </ClientsCell>
                    </ClientsRow>
                  ))}
                </tbody>
              </ClientsTable>
            )}
          </ClientsBody>
        </ClientsPanel>

        <SectionTitle>{t("tools.proxy.socksTitle")}</SectionTitle>
        <StatsGrid>
          <StatCard>
            <StatLabel>{t("tools.proxy.uptime")}</StatLabel>
            <StatValue>{formatUptime(socksStats.uptime)}</StatValue>
          </StatCard>
          <StatCard>
            <StatLabel>{t("tools.proxy.activeConnections")}</StatLabel>
            <StatValue>{socksStats.activeConnections}</StatValue>
          </StatCard>
          <StatCard>
            <StatLabel>{t("tools.proxy.totalConnections")}</StatLabel>
            <StatValue>{socksStats.totalConnections}</StatValue>
          </StatCard>
          <StatCard>
            <StatLabel>{t("tools.proxy.totalRequests")}</StatLabel>
            <StatValue>{socksStats.totalRequests}</StatValue>
          </StatCard>
          <StatCard>
            <StatLabel>{t("tools.proxy.bytesIn")}</StatLabel>
            <StatValue>{formatBytes(socksStats.bytesIn)}</StatValue>
          </StatCard>
          <StatCard>
            <StatLabel>{t("tools.proxy.bytesOut")}</StatLabel>
            <StatValue>{formatBytes(socksStats.bytesOut)}</StatValue>
          </StatCard>
        </StatsGrid>

        <ClientsPanel>
          <ClientsHeader>
            <ClientsTitle>{t("tools.proxy.socksClientsTitle")}</ClientsTitle>
            <span>
              {t("tools.proxy.clientsCount", {
                count: socksStats.clients.length,
              })}
            </span>
          </ClientsHeader>
          <ClientsBody>
            {socksStats.clients.length === 0 ? (
              <EmptyState>{t("tools.proxy.noClients")}</EmptyState>
            ) : (
              <ClientsTable>
                <ClientsHead>
                  <tr>
                    <th>{t("tools.proxy.table.ip")}</th>
                    <th>{t("tools.proxy.table.active")}</th>
                    <th>{t("tools.proxy.table.total")}</th>
                    <th>{t("tools.proxy.table.requests")}</th>
                    <th>{t("tools.proxy.table.bytesIn")}</th>
                    <th>{t("tools.proxy.table.bytesOut")}</th>
                    <th>{t("tools.proxy.table.lastSeen")}</th>
                    <th>{t("tools.proxy.table.topTargets")}</th>
                  </tr>
                </ClientsHead>
                <tbody>
                  {socksStats.clients.slice(0, 50).map((client) => (
                    <ClientsRow key={`socks-${client.ip}`}>
                      <ClientsCell>{client.ip}</ClientsCell>
                      <ClientsCell>{client.activeConnections}</ClientsCell>
                      <ClientsCell>{client.totalConnections}</ClientsCell>
                      <ClientsCell>{client.totalRequests}</ClientsCell>
                      <ClientsCell>{formatBytes(client.bytesIn)}</ClientsCell>
                      <ClientsCell>{formatBytes(client.bytesOut)}</ClientsCell>
                      <ClientsCell>{formatAge(client.lastSeenMs)}</ClientsCell>
                      <ClientsCell>
                        {formatTopTargets(client.topTargets)}
                      </ClientsCell>
                    </ClientsRow>
                  ))}
                </tbody>
              </ClientsTable>
            )}
          </ClientsBody>
        </ClientsPanel>
      </ContentPanel>
    </ProxyContainer>
  );
};

export default ProxyMonitor;
