import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import {
  LoadTestContainer,
  Header,
  Title,
  Subtitle,
  ConfigCard,
  FormGrid,
  FormRow,
  Label,
  TextInput,
  Textarea,
  ControlRow,
  InlineFields,
  MetricsGrid,
  MetricCard,
  MetricIcon,
  MetricValue,
  MetricLabel,
  MetricDelta,
  ChartsGrid,
  ChartCard,
  ChartHeader,
  ProgressBar,
  ProgressFill,
  StatusRow,
  Badge,
  InlineHint,
  HistoryBody,
  HistoryList,
  HistoryCard,
  HistoryOverlay,
  SectionHeader,
  SectionTitle,
  SectionAction,
  HeaderList,
  HeaderRow,
  HeaderInput,
} from "./LoadTest.styles";
import CustomSelect, {
  SelectOption,
} from "@/components/common/CustomSelect/CustomSelect";
import {
  ResponsiveContainer,
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip as RechartsTooltip,
  LineChart,
  Line,
  Legend,
} from "recharts";
import { StyledButton } from "@/components/styled/StyledButton";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import styled from "styled-components";
import {
  FiActivity,
  FiAlertTriangle,
  FiAperture,
  FiCheckCircle,
  FiClock,
  FiCpu,
  FiDatabase,
  FiLink,
  FiSend,
  FiTrendingUp,
  FiPlus,
  FiTrash2,
  FiDownloadCloud,
  FiChevronDown,
  FiChevronUp,
  FiHelpCircle,
  FiMoreHorizontal,
  FiRefreshCw,
  FiXCircle,
} from "react-icons/fi";

type DerivedMetrics = {
  successRate: number;
  avgLatency: number;
  requestsPerSecond: number;
  p50: number;
  p90: number;
  p95: number;
  p99: number;
  throughputPerSecond: number;
  throughputPerSecondUp: number;
  failureCount: number;
  totalRequests: number;
  totalBytes: number;
  totalBytesUp: number;
  sizePerRequest: number;
  sizePerRequestUp: number;
};

type LatencyPoint = {
  ts: number;
  latency: number;
};

type ThroughputPoint = {
  second: number;
  rps: number;
  successRate: number;
};

type MetricsEvent = {
  timestamp_ms: number;
  progress: number;
  total_requests: number;
  success: number;
  failures: number;
  avg_latency_ms: number;
  p50_ms: number;
  p90_ms: number;
  p95_ms: number;
  p99_ms: number;
  rps: number;
  throughput_bps: number;
  throughput_bps_up: number;
  total_bytes: number;
  total_bytes_up: number;
  avg_bytes_per_request: number;
  avg_bytes_per_request_up: number;
  status_codes: { code: number; count: number }[];
  status_no_response: number;
  status_other: number;
  completion_buckets: { percentile: number; latency_ms: number }[];
  done: boolean;
};

type StatusCodeStat = { code: number; count: number };

const MAX_LATENCY_POINTS = 80;
const HISTORY_PAGE_SIZE = 5;

const formatNumber = (value: number, fraction = 1) =>
  Number.isFinite(value) ? value.toFixed(fraction) : "0";

const formatBytes = (value: number) => {
  if (!Number.isFinite(value)) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let current = Math.max(0, value);
  let idx = 0;
  while (current >= 1024 && idx < units.length - 1) {
    current /= 1024;
    idx += 1;
  }
  const fraction = current >= 10 ? 0 : 1;
  return `${formatNumber(current, fraction)} ${units[idx]}`;
};

type HeaderItem = { id: number; key: string; value: string };

type HistoryEntry = {
  id: number;
  timestamp: number;
  url: string;
  method: string;
  concurrency: number;
  rampUpSecs?: number;
  iterationsPerWorker?: number | null;
  totalRequestsLimit?: number | null;
  duration: number;
  timeout: number;
  connectionMode: ConnectionMode;
  rpsLimit?: number | null;
  rpsMode?: RpsMode;
  headers?: Record<string, string>;
  summary: {
    totalRequests: number;
    successRate: number;
    avgLatency: number;
    rps: number;
    p50: number;
    p90: number;
    p95: number;
    p99: number;
  };
};

type HistoryPage = {
  total: number;
  items: HistoryEntry[];
};

type ResponseMode = "countBytes" | "discardBody";
type MetricsMode = "full" | "minimal";
type ConnectionMode = "keepAlive" | "newConnection";
type RpsMode = "global" | "perWorker";

const SwitchContainer = styled.label`
  display: flex;
  align-items: center;
  cursor: pointer;
  gap: 10px;
  line-height: 1;
`;

const SwitchInput = styled.input`
  opacity: 0;
  width: 0;
  height: 0;
`;

const SwitchSlider = styled.span`
  position: relative;
  display: inline-block;
  width: 40px;
  height: 22px;
  flex: 0 0 auto;
  background-color: ${(props) => props.theme.colors.border};
  border-radius: 22px;
  transition: background-color 0.2s;

  &::before {
    content: "";
    position: absolute;
    height: 18px;
    width: 18px;
    left: 2px;
    bottom: 2px;
    background-color: white;
    border-radius: 50%;
    transition: transform 0.2s;
  }

  ${SwitchInput}:checked + & {
    background-color: ${(props) => props.theme.colors.primary};
  }

  ${SwitchInput}:checked + &::before {
    transform: translateX(18px);
  }
`;

const SwitchField = styled.div<{ $disabled?: boolean }>`
  width: 100%;
  padding: 12px;
  border-radius: ${(props) => props.theme.radii.base};
  border: 1px solid ${(props) => props.theme.colors.border};
  background: ${(props) => props.theme.colors.background};
  color: ${(props) => props.theme.colors.textPrimary};
  display: flex;
  align-items: center;
  min-height: 44px;
  opacity: ${(props) => (props.$disabled ? 0.6 : 1)};
`;

const LoadTest: React.FC = () => {
  const { t } = useTranslation();
  const [url, setUrl] = useState("http://127.0.0.1");
  const [method, setMethod] = useState<"GET" | "POST">("GET");
  const [concurrencyInput, setConcurrencyInput] = useState("30");
  const [rampUpInput, setRampUpInput] = useState("0");
  const [iterationsPerWorkerInput, setIterationsPerWorkerInput] = useState("");
  const [totalRequestsLimitInput, setTotalRequestsLimitInput] = useState("");
  const [durationInput, setDurationInput] = useState("10");
  const [timeoutInput, setTimeoutInput] = useState("10");
  const [rpsInput, setRpsInput] = useState("");
  const [responseMode, setResponseMode] = useState<ResponseMode>("countBytes");
  const [metricsMode, setMetricsMode] = useState<MetricsMode>("full");
  const [connectionMode, setConnectionMode] =
    useState<ConnectionMode>("keepAlive");
  const [rpsMode, setRpsMode] = useState<RpsMode>("global");
  const [allowInsecureCerts, setAllowInsecureCerts] = useState(false);
  const [payload, setPayload] = useState("");
  const [headers, setHeaders] = useState<HeaderItem[]>([
    { id: 1, key: "", value: "" },
  ]);
  const methodOptions = useMemo<SelectOption[]>(
    () => [
      { value: "GET", label: t("tools.loadtest.methodGet") },
      { value: "POST", label: t("tools.loadtest.methodPost") },
    ],
    [t],
  );
  const rpsModeOptions = useMemo<SelectOption[]>(
    () => [
      { value: "global", label: t("tools.loadtest.rpsModeGlobal") },
      { value: "perWorker", label: t("tools.loadtest.rpsModePerWorker") },
    ],
    [t],
  );
  const responseModeOptions = useMemo<SelectOption[]>(
    () => [
      {
        value: "countBytes",
        label: t("tools.loadtest.responseModeCountBytes"),
      },
      {
        value: "discardBody",
        label: t("tools.loadtest.responseModeDiscardBody"),
      },
    ],
    [t],
  );
  const metricsModeOptions = useMemo<SelectOption[]>(
    () => [
      { value: "full", label: t("tools.loadtest.metricsModeFull") },
      { value: "minimal", label: t("tools.loadtest.metricsModeMinimal") },
    ],
    [t],
  );
  const connectionModeOptions = useMemo<SelectOption[]>(
    () => [
      {
        value: "keepAlive",
        label: t("tools.loadtest.connectionModeKeepAlive"),
      },
      {
        value: "newConnection",
        label: t("tools.loadtest.connectionModeNewConnection"),
      },
    ],
    [t],
  );
  const [requestConfigOpen, setRequestConfigOpen] = useState(false);
  const [isRunning, setIsRunning] = useState(false);
  const [progress, setProgress] = useState(0);
  const [lastError, setLastError] = useState<string | null>(null);

  const [visibleMetrics, setVisibleMetrics] = useState<DerivedMetrics>({
    successRate: 0,
    avgLatency: 0,
    requestsPerSecond: 0,
    p50: 0,
    p90: 0,
    p95: 0,
    p99: 0,
    throughputPerSecond: 0,
    throughputPerSecondUp: 0,
    failureCount: 0,
    totalRequests: 0,
    totalBytes: 0,
    totalBytesUp: 0,
    sizePerRequest: 0,
    sizePerRequestUp: 0,
  });
  const [latencySeries, setLatencySeries] = useState<LatencyPoint[]>([]);
  const [throughputSeries, setThroughputSeries] = useState<ThroughputPoint[]>(
    [],
  );
  const [statusCodes, setStatusCodes] = useState<StatusCodeStat[]>([]);
  const [statusNoResponse, setStatusNoResponse] = useState(0);
  const [statusOther, setStatusOther] = useState(0);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [totalHistory, setTotalHistory] = useState(0);
  const [currentPage, setCurrentPage] = useState(1);
  const [historyLoading, setHistoryLoading] = useState(false);
  const pageRequestSum = history.reduce(
    (sum, item) => sum + item.summary.totalRequests,
    0,
  );
  const renderConnectionMode = useCallback(
    (mode: ConnectionMode) =>
      mode === "newConnection"
        ? t("tools.loadtest.connectionModeTagNew")
        : t("tools.loadtest.connectionModeTagKeepAlive"),
    [t],
  );
  const renderRpsMode = useCallback(
    (mode?: RpsMode) =>
      mode === "perWorker"
        ? t("tools.loadtest.rpsModePerWorker")
        : t("tools.loadtest.rpsModeGlobal"),
    [t],
  );

  const runningRef = useRef(false);
  const completedRef = useRef(false);
  const prevRunningRef = useRef(false);

  const concurrencyValue = Math.min(
    5000,
    Math.max(1, Number.parseInt(concurrencyInput, 10) || 1),
  );
  const durationValue = Math.min(
    600,
    Math.max(1, Number.parseInt(durationInput, 10) || 1),
  );
  const rampUpValue = Math.min(
    durationValue,
    Math.max(0, Number.parseInt(rampUpInput, 10) || 0),
  );
  const iterationsPerWorkerValue = Math.min(
    1_000_000,
    Math.max(0, Number.parseInt(iterationsPerWorkerInput, 10) || 0),
  );
  const totalRequestsLimitValue = Math.min(
    10_000_000,
    Math.max(0, Number.parseInt(totalRequestsLimitInput, 10) || 0),
  );
  const timeoutValue = Math.min(
    120,
    Math.max(1, Number.parseInt(timeoutInput, 10) || 1),
  );
  const rpsValueRaw = Number.parseFloat(rpsInput);
  const rpsValue = Number.isFinite(rpsValueRaw)
    ? Math.min(1_000_000, Math.max(0, rpsValueRaw))
    : 0;
  const rpsLimit = rpsValue > 0 ? rpsValue : undefined;
  const iterationsPerWorker =
    iterationsPerWorkerValue > 0 ? iterationsPerWorkerValue : undefined;
  const totalRequestsLimit =
    totalRequestsLimitValue > 0 ? totalRequestsLimitValue : undefined;
  const isHttpsTarget = url.trim().startsWith("https://");

  const resetState = useCallback(() => {
    setLatencySeries([]);
    setThroughputSeries([]);
    setVisibleMetrics({
      successRate: 0,
      avgLatency: 0,
      requestsPerSecond: 0,
      p50: 0,
      p90: 0,
      p95: 0,
      p99: 0,
      throughputPerSecond: 0,
      throughputPerSecondUp: 0,
      failureCount: 0,
      totalRequests: 0,
      totalBytes: 0,
      totalBytesUp: 0,
      sizePerRequest: 0,
      sizePerRequestUp: 0,
    });
    setProgress(0);
    setStatusCodes([]);
    setStatusNoResponse(0);
    setStatusOther(0);
  }, []);

  const cleanHeaders = useCallback(() => {
    const entries = headers
      .map((h) => ({ key: h.key.trim(), value: h.value.trim() }))
      .filter((h) => h.key.length > 0);
    if (entries.length === 0) return undefined;
    const result: Record<string, string> = {};
    entries.forEach((h) => {
      result[h.key] = h.value;
    });
    return result;
  }, [headers]);

  const fetchHistory = useCallback(async (page = 1) => {
    setHistoryLoading(true);
    try {
      const result = await invoke<HistoryPage>("list_load_test_history", {
        page,
        pageSize: HISTORY_PAGE_SIZE,
      });
      setHistory(result.items);
      setTotalHistory(result.total);
      setCurrentPage(page);
    } catch (error) {
      setLastError(error instanceof Error ? error.message : String(error));
    } finally {
      setHistoryLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchHistory(1);
  }, [fetchHistory]);

  const handleMetricsEvent = useCallback(
    (payload: MetricsEvent) => {
      if (!runningRef.current && !payload.done) {
        return;
      }

      setVisibleMetrics({
        successRate:
          payload.total_requests === 0
            ? 0
            : (payload.success / payload.total_requests) * 100,
        avgLatency: payload.avg_latency_ms,
        requestsPerSecond: payload.rps,
        p50: payload.p50_ms,
        p90: payload.p90_ms,
        p95: payload.p95_ms,
        p99: payload.p99_ms,
        throughputPerSecond: payload.throughput_bps,
        throughputPerSecondUp: payload.throughput_bps_up,
        failureCount: payload.failures,
        totalRequests: payload.total_requests,
        totalBytes: payload.total_bytes,
        totalBytesUp: payload.total_bytes_up,
        sizePerRequest: payload.avg_bytes_per_request,
        sizePerRequestUp: payload.avg_bytes_per_request_up,
      });

      setProgress(payload.progress * 100);
      const sortedStatuses = [...payload.status_codes].sort(
        (a, b) => b.count - a.count,
      );
      setStatusCodes(sortedStatuses);
      setStatusNoResponse(payload.status_no_response);
      setStatusOther(payload.status_other);

      setLatencySeries((prev) => {
        const next = [
          ...prev,
          { ts: payload.timestamp_ms, latency: payload.avg_latency_ms },
        ];
        return next.length > MAX_LATENCY_POINTS
          ? next.slice(next.length - MAX_LATENCY_POINTS)
          : next;
      });

      setThroughputSeries((prev) => {
        const next = [
          ...prev,
          {
            second: Math.round(payload.timestamp_ms / 1000),
            rps: payload.rps,
            successRate:
              payload.total_requests === 0
                ? 0
                : (payload.success / payload.total_requests) * 100,
          },
        ];
        return next.length > 120 ? next.slice(next.length - 120) : next;
      });

      if (payload.done) {
        if (completedRef.current) {
          return;
        }
        completedRef.current = true;
        runningRef.current = false;
        setIsRunning(false);
        fetchHistory(1);
      }
    },
    [
      cleanHeaders,
      concurrencyValue,
      durationValue,
      fetchHistory,
      method,
      timeoutValue,
      url,
    ],
  );

  useEffect(() => {
    const unlistenMetricsPromise = listen<MetricsEvent>(
      "loadtest:metrics",
      (event) => handleMetricsEvent(event.payload),
    );
    const unlistenLogPromise = listen<string>("loadtest:log", (event) => {
      setLastError(event.payload);
    });

    return () => {
      unlistenMetricsPromise.then((un) => un());
      unlistenLogPromise.then((un) => un());
    };
  }, [handleMetricsEvent]);

  useEffect(() => {
    if (!isRunning && prevRunningRef.current) {
      fetchHistory(1);
    }
    prevRunningRef.current = isRunning;
  }, [isRunning, fetchHistory]);

  const handleStart = useCallback(async () => {
    if (!url.startsWith("http")) {
      setLastError(t("tools.loadtest.error.invalidUrl"));
      return;
    }

    resetState();
    runningRef.current = true;
    completedRef.current = false;
    setIsRunning(true);
    setLastError(null);

    try {
      await invoke("start_load_test", {
        config: {
          url,
          method,
          concurrency: concurrencyValue,
          durationSecs: durationValue,
          duration_secs: durationValue,
          timeoutMs: timeoutValue * 1000,
          timeout_ms: timeoutValue * 1000,
          rps: rpsLimit ?? null,
          rpsMode,
          rps_mode: rpsMode,
          payload: method === "GET" ? null : payload,
          headers: cleanHeaders(),
          responseMode,
          response_mode: responseMode,
          metricsMode,
          metrics_mode: metricsMode,
          connectionMode,
          connection_mode: connectionMode,
          allowInsecureCerts,
          allow_insecure_certs: allowInsecureCerts,
          rampUpSecs: rampUpValue,
          ramp_up_secs: rampUpValue,
          iterationsPerWorker: iterationsPerWorker ?? null,
          iterations_per_worker: iterationsPerWorker ?? null,
          totalRequestsLimit: totalRequestsLimit ?? null,
          total_requests_limit: totalRequestsLimit ?? null,
        },
      });
    } catch (error) {
      runningRef.current = false;
      setIsRunning(false);
      setLastError(error instanceof Error ? error.message : String(error));
    }
  }, [
    cleanHeaders,
    concurrencyValue,
    durationValue,
    method,
    payload,
    resetState,
    responseMode,
    metricsMode,
    connectionMode,
    rpsLimit,
    rpsMode,
    allowInsecureCerts,
    rampUpValue,
    iterationsPerWorker,
    totalRequestsLimit,
    t,
    timeoutValue,
    url,
  ]);

  const handleAddHeader = useCallback(() => {
    setHeaders((prev) => [...prev, { id: Date.now(), key: "", value: "" }]);
  }, []);

  const handleHeaderChange = useCallback(
    (id: number, field: "key" | "value", value: string) => {
      setHeaders((prev) =>
        prev.map((item) =>
          item.id === id ? { ...item, [field]: value } : item,
        ),
      );
    },
    [],
  );

  const handleRemoveHeader = useCallback((id: number) => {
    setHeaders((prev) => prev.filter((item) => item.id !== id));
  }, []);

  const handleStop = useCallback(async () => {
    runningRef.current = false;
    completedRef.current = true;
    setIsRunning(false);
    try {
      await invoke("stop_load_test");
    } catch (error) {
      setLastError(error instanceof Error ? error.message : String(error));
    }
  }, []);

  const statusText = isRunning
    ? t("tools.loadtest.status.running", {
        seconds: durationValue,
        concurrency: concurrencyValue,
      })
    : t("tools.loadtest.status.idle");

  const handleExportHistory = useCallback(async () => {
    if (totalHistory === 0) {
      return;
    }
    try {
      const records = await invoke<HistoryEntry[]>("export_load_test_history");
      if (records.length === 0) return;
      const defaultPath = `loadtest-history-${Date.now()}.json`;
      const targetPath = await save({
        defaultPath,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!targetPath) return;
      await writeTextFile(targetPath, JSON.stringify(records, null, 2));
    } catch (error) {
      setLastError(error instanceof Error ? error.message : String(error));
    }
  }, [totalHistory]);

  const handleClearHistory = useCallback(async () => {
    if (historyLoading || totalHistory === 0) return;
    setHistoryLoading(true);
    try {
      await invoke("clear_load_test_history");
      setHistory([]);
      setTotalHistory(0);
      setCurrentPage(1);
    } catch (error) {
      setLastError(error instanceof Error ? error.message : String(error));
    } finally {
      setHistoryLoading(false);
    }
  }, [historyLoading, totalHistory]);

  const statusSummary = useMemo(() => {
    let status2xx = 0;
    let status3xx = 0;
    let status4xx = 0;
    let status5xx = 0;
    let statusOtherCodes = statusOther;
    for (const item of statusCodes) {
      const code = item.code;
      if (code >= 200 && code < 300) {
        status2xx += item.count;
      } else if (code >= 300 && code < 400) {
        status3xx += item.count;
      } else if (code >= 400 && code < 500) {
        status4xx += item.count;
      } else if (code >= 500 && code < 600) {
        status5xx += item.count;
      } else {
        statusOtherCodes += item.count;
      }
    }
    const total =
      status2xx +
      status3xx +
      status4xx +
      status5xx +
      statusOtherCodes +
      statusNoResponse;
    return {
      total,
      items: [
        {
          key: "2xx",
          label: "2xx",
          count: status2xx,
          color: "#10b981",
          Icon: FiCheckCircle,
        },
        {
          key: "3xx",
          label: "3xx",
          count: status3xx,
          color: "#0ea5e9",
          Icon: FiRefreshCw,
        },
        {
          key: "4xx",
          label: "4xx",
          count: status4xx,
          color: "#f59e0b",
          Icon: FiAlertTriangle,
        },
        {
          key: "5xx",
          label: "5xx",
          count: status5xx,
          color: "#ef4444",
          Icon: FiXCircle,
        },
        {
          key: "noResponse",
          label: t("tools.loadtest.metrics.statusNoResponse"),
          count: statusNoResponse,
          color: "#64748b",
          Icon: FiHelpCircle,
        },
        {
          key: "other",
          label: t("tools.loadtest.metrics.statusOther"),
          count: statusOtherCodes,
          color: "#94a3b8",
          Icon: FiMoreHorizontal,
        },
      ],
    };
  }, [statusCodes, statusNoResponse, statusOther, t]);

  return (
    <LoadTestContainer>
      <Header>
        <Title>
          <FiActivity />
          {t("tools.loadtest.title")}
        </Title>
        <Subtitle>{t("tools.loadtest.subtitle")}</Subtitle>
      </Header>

      <ConfigCard>
        <SectionHeader>
          <SectionTitle>
            <FiAperture />
            {t("tools.loadtest.baseConfig")}
          </SectionTitle>
          <Badge $tone={isRunning ? "success" : undefined}>{statusText}</Badge>
        </SectionHeader>
        <FormGrid>
          <FormRow>
            <Label htmlFor="url">
              <FiSend />
              {t("tools.loadtest.targetUrl")}
            </Label>
            <TextInput
              id="url"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="https://example.com/api"
              disabled={isRunning}
            />
          </FormRow>
          <FormRow>
            <Label htmlFor="method">
              <FiAperture />
              {t("tools.loadtest.method")}
            </Label>
            <CustomSelect
              id="method"
              value={method}
              options={methodOptions}
              onChange={(value) => setMethod(value as "GET" | "POST")}
              disabled={isRunning}
            />
          </FormRow>
          <FormRow>
            <Label htmlFor="concurrency">
              <FiCpu />
              {t("tools.loadtest.concurrency")}
            </Label>
            <TextInput
              id="concurrency"
              type="number"
              min={1}
              max={5000}
              value={concurrencyInput}
              onChange={(e) => setConcurrencyInput(e.target.value)}
              onBlur={(e) =>
                setConcurrencyInput(
                  Math.min(
                    5000,
                    Math.max(1, Number.parseInt(e.target.value, 10) || 1),
                  ).toString(),
                )
              }
              disabled={isRunning}
            />
          </FormRow>
          <FormRow>
            <Label htmlFor="rampUp">
              <FiClock />
              {t("tools.loadtest.rampUp")}
            </Label>
            <TextInput
              id="rampUp"
              type="number"
              min={0}
              max={durationValue}
              value={rampUpInput}
              onChange={(e) => setRampUpInput(e.target.value)}
              onBlur={(e) =>
                setRampUpInput(
                  Math.min(
                    durationValue,
                    Math.max(0, Number.parseInt(e.target.value, 10) || 0),
                  ).toString(),
                )
              }
              disabled={isRunning}
            />
          </FormRow>
          <FormRow>
            <Label htmlFor="iterationsPerWorker">
              <FiRefreshCw />
              {t("tools.loadtest.iterationsPerWorker")}
            </Label>
            <TextInput
              id="iterationsPerWorker"
              type="number"
              min={0}
              max={1_000_000}
              value={iterationsPerWorkerInput}
              onChange={(e) => setIterationsPerWorkerInput(e.target.value)}
              onBlur={(e) =>
                setIterationsPerWorkerInput(
                  Math.min(
                    1_000_000,
                    Math.max(0, Number.parseInt(e.target.value, 10) || 0),
                  ).toString(),
                )
              }
              disabled={isRunning}
              placeholder={t("tools.loadtest.rpsUnlimitedShort")}
            />
          </FormRow>
          <FormRow>
            <Label htmlFor="totalRequestsLimit">
              <FiDatabase />
              {t("tools.loadtest.totalRequestsLimit")}
            </Label>
            <TextInput
              id="totalRequestsLimit"
              type="number"
              min={0}
              max={10_000_000}
              value={totalRequestsLimitInput}
              onChange={(e) => setTotalRequestsLimitInput(e.target.value)}
              onBlur={(e) =>
                setTotalRequestsLimitInput(
                  Math.min(
                    10_000_000,
                    Math.max(0, Number.parseInt(e.target.value, 10) || 0),
                  ).toString(),
                )
              }
              disabled={isRunning}
              placeholder={t("tools.loadtest.rpsUnlimitedShort")}
            />
          </FormRow>
          <FormRow>
            <Label htmlFor="duration">
              <FiClock />
              {t("tools.loadtest.duration")}
            </Label>
            <TextInput
              id="duration"
              type="number"
              min={1}
              max={600}
              value={durationInput}
              onChange={(e) => setDurationInput(e.target.value)}
              onBlur={(e) =>
                setDurationInput(
                  Math.min(
                    600,
                    Math.max(1, Number.parseInt(e.target.value, 10) || 1),
                  ).toString(),
                )
              }
              disabled={isRunning}
            />
          </FormRow>
          <FormRow>
            <Label htmlFor="timeout">
              <FiAlertTriangle />
              {t("tools.loadtest.timeout")}
            </Label>
            <TextInput
              id="timeout"
              type="number"
              min={1}
              max={120}
              value={timeoutInput}
              onChange={(e) => setTimeoutInput(e.target.value)}
              onBlur={(e) =>
                setTimeoutInput(
                  Math.min(
                    120,
                    Math.max(1, Number.parseInt(e.target.value, 10) || 1),
                  ).toString(),
                )
              }
              disabled={isRunning}
            />
          </FormRow>
          <FormRow>
            <Label htmlFor="rpsLimit">
              <FiTrendingUp />
              {t("tools.loadtest.rpsLimit")}
            </Label>
            <TextInput
              id="rpsLimit"
              type="number"
              min={0}
              max={1_000_000}
              step="0.1"
              value={rpsInput}
              onChange={(e) => setRpsInput(e.target.value)}
              onBlur={(e) => {
                const parsed = Number.parseFloat(e.target.value);
                if (!Number.isFinite(parsed) || parsed <= 0) {
                  setRpsInput("");
                  return;
                }
                const clamped = Math.min(1_000_000, Math.max(0.1, parsed));
                setRpsInput(clamped.toString());
              }}
              placeholder={t("tools.loadtest.rpsUnlimited")}
              disabled={isRunning}
            />
          </FormRow>
          <FormRow>
            <Label htmlFor="rpsMode">
              <FiTrendingUp />
              {t("tools.loadtest.rpsMode")}
            </Label>
            <CustomSelect
              id="rpsMode"
              value={rpsMode}
              options={rpsModeOptions}
              onChange={(value) => setRpsMode(value as RpsMode)}
              disabled={isRunning}
            />
          </FormRow>
          <FormRow>
            <Label htmlFor="responseMode">
              <FiCpu />
              {t("tools.loadtest.responseMode")}
            </Label>
            <CustomSelect
              id="responseMode"
              value={responseMode}
              options={responseModeOptions}
              onChange={(value) => setResponseMode(value as ResponseMode)}
              disabled={isRunning}
            />
          </FormRow>
          <FormRow>
            <Label htmlFor="metricsMode">
              <FiActivity />
              {t("tools.loadtest.metricsMode")}
            </Label>
            <CustomSelect
              id="metricsMode"
              value={metricsMode}
              options={metricsModeOptions}
              onChange={(value) => setMetricsMode(value as MetricsMode)}
              disabled={isRunning}
            />
          </FormRow>
          <FormRow>
            <Label htmlFor="connectionMode">
              <FiLink />
              {t("tools.loadtest.connectionMode")}
            </Label>
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              <CustomSelect
                id="connectionMode"
                value={connectionMode}
                options={connectionModeOptions}
                onChange={(value) => setConnectionMode(value as ConnectionMode)}
                disabled={isRunning}
              />
            </div>
          </FormRow>
          <FormRow>
            <Label htmlFor="allowInsecureCerts">
              <FiAlertTriangle />
              {t("tools.loadtest.allowInsecureCerts")}
            </Label>
            <SwitchField $disabled={isRunning || !isHttpsTarget}>
              <SwitchContainer
                style={{
                  cursor:
                    isRunning || !isHttpsTarget ? "not-allowed" : "pointer",
                }}
              >
                <SwitchInput
                  id="allowInsecureCerts"
                  type="checkbox"
                  checked={allowInsecureCerts}
                  onChange={(e) => setAllowInsecureCerts(e.target.checked)}
                  disabled={isRunning || !isHttpsTarget}
                />
                <SwitchSlider />
              </SwitchContainer>
            </SwitchField>
          </FormRow>
        </FormGrid>

        <ControlRow>
          <InlineFields>
            <InlineHint>
              {t("tools.loadtest.progress")}: {formatNumber(progress, 0)}%
            </InlineHint>
            <StatusRow>
              {lastError && (
                <Badge $tone="warning">
                  {t("tools.loadtest.lastError")}: {lastError}
                </Badge>
              )}
            </StatusRow>
          </InlineFields>
          <div style={{ display: "flex", gap: 12 }}>
            <StyledButton
              variant="primary"
              whileTap={{ scale: 0.96 }}
              onClick={handleStart}
              disabled={isRunning}
            >
              <FiActivity />
              {t("tools.loadtest.start")}
            </StyledButton>
            <StyledButton
              variant="danger"
              whileTap={{ scale: 0.96 }}
              onClick={handleStop}
              disabled={!isRunning}
            >
              <FiAlertTriangle />
              {t("tools.loadtest.stop")}
            </StyledButton>
          </div>
        </ControlRow>
        <ProgressBar>
          <ProgressFill
            initial={{ width: 0 }}
            animate={{ width: `${progress}%` }}
            transition={{ duration: 0.3 }}
          />
        </ProgressBar>
      </ConfigCard>

      <ConfigCard>
        <SectionHeader>
          <SectionTitle>
            <FiAperture />
            {t("tools.loadtest.requestConfig")}
          </SectionTitle>
          <div style={{ display: "flex", gap: 12 }}>
            <SectionAction
              onClick={() => setRequestConfigOpen((prev) => !prev)}
              disabled={isRunning}
            >
              {requestConfigOpen ? <FiChevronUp /> : <FiChevronDown />}
              {requestConfigOpen
                ? t("tools.loadtest.closeRequestConfig")
                : t("tools.loadtest.openRequestConfig")}
            </SectionAction>
            {requestConfigOpen && (
              <SectionAction onClick={handleAddHeader} disabled={isRunning}>
                <FiPlus /> {t("tools.loadtest.addHeader")}
              </SectionAction>
            )}
          </div>
        </SectionHeader>
        {requestConfigOpen && (
          <FormGrid>
            <FormRow>
              <Label>{t("tools.loadtest.headers")}</Label>
              <HeaderList>
                {headers.map((item) => (
                  <HeaderRow key={item.id}>
                    <HeaderInput
                      placeholder="Authorization"
                      value={item.key}
                      onChange={(e) =>
                        handleHeaderChange(item.id, "key", e.target.value)
                      }
                      disabled={isRunning}
                    />
                    <HeaderInput
                      placeholder="Bearer xxx"
                      value={item.value}
                      onChange={(e) =>
                        handleHeaderChange(item.id, "value", e.target.value)
                      }
                      disabled={isRunning}
                    />
                    <StyledButton
                      variant="ghost"
                      whileTap={{ scale: 0.95 }}
                      onClick={() => handleRemoveHeader(item.id)}
                      disabled={isRunning || headers.length <= 1}
                    >
                      <FiTrash2 />
                    </StyledButton>
                  </HeaderRow>
                ))}
              </HeaderList>
            </FormRow>
            <FormRow>
              <Label htmlFor="payload">
                <FiTrendingUp />
                {t("tools.loadtest.payload")}
              </Label>
              <Textarea
                id="payload"
                value={payload}
                onChange={(e) => setPayload(e.target.value)}
                placeholder='{"message":"hello"}'
                disabled={isRunning || method === "GET"}
              />
            </FormRow>
          </FormGrid>
        )}
      </ConfigCard>

      <MetricsGrid>
        <MetricCard>
          <MetricIcon $color="#10b981">
            <FiCheckCircle />
          </MetricIcon>
          <div>
            <MetricLabel>{t("tools.loadtest.metrics.successRate")}</MetricLabel>
            <MetricValue>
              {formatNumber(visibleMetrics.successRate, 2)}%
            </MetricValue>
            <MetricDelta $positive>
              {t("tools.loadtest.metrics.totalRequests")}:{" "}
              {visibleMetrics.totalRequests}
            </MetricDelta>
          </div>
        </MetricCard>
        <MetricCard>
          <MetricIcon $color="#0ea5e9">
            <FiClock />
          </MetricIcon>
          <div>
            <MetricLabel>{t("tools.loadtest.metrics.avgLatency")}</MetricLabel>
            <MetricValue>
              {formatNumber(visibleMetrics.avgLatency, 1)} ms
            </MetricValue>
            <MetricDelta>
              P50 {formatNumber(visibleMetrics.p50, 1)} ms | P90{" "}
              {formatNumber(visibleMetrics.p90, 1)} ms
            </MetricDelta>
            <MetricDelta>
              P95 {formatNumber(visibleMetrics.p95, 1)} ms | P99{" "}
              {formatNumber(visibleMetrics.p99, 1)} ms
            </MetricDelta>
          </div>
        </MetricCard>
        <MetricCard>
          <MetricIcon $color="#6366f1">
            <FiTrendingUp />
          </MetricIcon>
          <div>
            <MetricLabel>{t("tools.loadtest.metrics.rps")}</MetricLabel>
            <MetricValue>
              {formatNumber(visibleMetrics.requestsPerSecond, 1)} req/s
            </MetricValue>
            <MetricDelta $positive>
              {t("tools.loadtest.metrics.totalThroughput")}:{" "}
              {formatBytes(
                visibleMetrics.throughputPerSecond +
                  visibleMetrics.throughputPerSecondUp,
              )}
              /s
            </MetricDelta>
            <MetricDelta>
              {t("tools.loadtest.metrics.sizePerRequest")}:{" "}
              {formatBytes(visibleMetrics.sizePerRequest)} /{" "}
              {formatBytes(visibleMetrics.sizePerRequestUp)}
            </MetricDelta>
          </div>
        </MetricCard>
        <MetricCard>
          <MetricIcon $color="#0ea5e9">
            <FiDatabase />
          </MetricIcon>
          <div>
            <MetricLabel>{t("tools.loadtest.metrics.totalData")}</MetricLabel>
            <MetricValue>
              {formatBytes(
                visibleMetrics.totalBytes + visibleMetrics.totalBytesUp,
              )}
            </MetricValue>
            <MetricDelta>
              {t("tools.loadtest.metrics.download")}:{" "}
              {formatBytes(visibleMetrics.totalBytes)} ·{" "}
              {t("tools.loadtest.metrics.upload")}:{" "}
              {formatBytes(visibleMetrics.totalBytesUp)}
            </MetricDelta>
            <MetricDelta>
              {t("tools.loadtest.metrics.totalThroughput")}:{" "}
              {formatBytes(
                visibleMetrics.throughputPerSecond +
                  visibleMetrics.throughputPerSecondUp,
              )}
              /s
            </MetricDelta>
          </div>
        </MetricCard>
        <MetricCard>
          <MetricIcon $color="#f59e0b">
            <FiAlertTriangle />
          </MetricIcon>
          <div>
            <MetricLabel>{t("tools.loadtest.metrics.failures")}</MetricLabel>
            <MetricValue>{visibleMetrics.failureCount}</MetricValue>
            <MetricDelta>
              {t("tools.loadtest.metrics.totalRequests")}:{" "}
              {visibleMetrics.totalRequests}
            </MetricDelta>
          </div>
        </MetricCard>
      </MetricsGrid>

      <ChartsGrid>
        <ChartCard>
          <ChartHeader>
            <span>{t("tools.loadtest.chart.latency")}</span>
            <Badge>{t("tools.loadtest.chart.legendLatency")}</Badge>
          </ChartHeader>
          <ResponsiveContainer width="100%" height={240}>
            <LineChart data={latencySeries}>
              <CartesianGrid strokeDasharray="3 3" />
              <XAxis
                dataKey="ts"
                tickFormatter={() => ""}
                tick={{ fontSize: 12 }}
              />
              <YAxis
                tick={{ fontSize: 12 }}
                label={{
                  value: "ms",
                  angle: -90,
                  position: "insideLeft",
                }}
              />
              <RechartsTooltip
                formatter={(value: number) => `${formatNumber(value, 1)} ms`}
                labelFormatter={() => ""}
              />
              <Line
                type="monotone"
                dataKey="latency"
                stroke="#0ea5e9"
                strokeWidth={2}
                dot={false}
              />
            </LineChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard>
          <ChartHeader>
            <span>{t("tools.loadtest.chart.requests")}</span>
            <Badge>{t("tools.loadtest.chart.legendRps")}</Badge>
          </ChartHeader>
          <ResponsiveContainer width="100%" height={240}>
            <AreaChart data={throughputSeries}>
              <CartesianGrid strokeDasharray="3 3" />
              <XAxis
                dataKey="second"
                tick={{ fontSize: 12 }}
                tickFormatter={(value) => `${value}s`}
              />
              <YAxis tick={{ fontSize: 12 }} />
              <RechartsTooltip
                formatter={(value: number, name) =>
                  name === "successRate"
                    ? `${formatNumber(value, 2)}%`
                    : formatNumber(value, 1)
                }
                labelFormatter={(label) => `${label}s`}
              />
              <Legend />
              <Area
                type="monotone"
                dataKey="rps"
                name={t("tools.loadtest.chart.legendRps")}
                stroke="#10b981"
                fill="#10b98122"
                strokeWidth={2}
              />
              <Line
                type="monotone"
                dataKey="successRate"
                name={t("tools.loadtest.chart.legendSuccess")}
                stroke="#6366f1"
                strokeWidth={2}
                dot={false}
              />
            </AreaChart>
          </ResponsiveContainer>
        </ChartCard>
        <ChartCard>
          <ChartHeader>
            <span>{t("tools.loadtest.metrics.statusDistribution")}</span>
            <Badge>{t("tools.loadtest.metrics.statusDistribution")}</Badge>
          </ChartHeader>
          {statusSummary.total > 0 ? (
            <div
              style={{
                display: "grid",
                gridTemplateColumns: "repeat(auto-fit, minmax(140px, 1fr))",
                gap: 12,
              }}
            >
              {statusSummary.items
                .filter((item) => item.count > 0)
                .map((item) => {
                  const percent =
                    statusSummary.total > 0
                      ? (item.count / statusSummary.total) * 100
                      : 0;
                  return (
                    <div
                      key={item.key}
                      style={{
                        border: "1px solid var(--border, rgba(0,0,0,0.08))",
                        borderRadius: 8,
                        padding: "10px 12px",
                      }}
                    >
                      <div
                        style={{
                          display: "flex",
                          alignItems: "center",
                          gap: 6,
                          color: item.color,
                          fontWeight: 700,
                        }}
                      >
                        <item.Icon />
                        {item.label}
                      </div>
                      <div style={{ color: "gray" }}>
                        {item.count} · {formatNumber(percent, 1)}%
                      </div>
                    </div>
                  );
                })}
            </div>
          ) : (
            <InlineHint>{t("tools.loadtest.metrics.noStatusData")}</InlineHint>
          )}
        </ChartCard>
      </ChartsGrid>

      <ConfigCard>
        <SectionHeader>
          <SectionTitle>
            <FiClock />
            {t("tools.loadtest.history")}
          </SectionTitle>
          <InlineHint>
            {t("tools.loadtest.historyStats", {
              total: totalHistory,
              pageRequests: pageRequestSum,
            })}
          </InlineHint>
          <div style={{ display: "flex", gap: 12 }}>
            <SectionAction
              onClick={handleClearHistory}
              disabled={historyLoading || totalHistory === 0}
            >
              <FiTrash2 />
              {t("tools.loadtest.clearHistory")}
            </SectionAction>
            <SectionAction
              onClick={handleExportHistory}
              disabled={historyLoading || totalHistory === 0}
            >
              <FiDownloadCloud />
              {t("tools.loadtest.exportHistory")}
            </SectionAction>
          </div>
        </SectionHeader>
        <HistoryBody>
          {history.length === 0 && !historyLoading ? (
            <InlineHint>{t("tools.loadtest.historyEmpty")}</InlineHint>
          ) : (
            <>
              <HistoryList>
                {history.map((item) => (
                  <HistoryCard key={item.id}>
                    <div style={{ fontWeight: 700 }}>
                      {new Date(item.timestamp).toLocaleString()} -{" "}
                      {item.method} {item.url}
                    </div>
                    <div style={{ color: "gray" }}>
                      {t("tools.loadtest.concurrency")}: {item.concurrency} -{" "}
                      {t("tools.loadtest.duration")}: {item.duration}s -{" "}
                      {t("tools.loadtest.connectionMode")}:{" "}
                      {renderConnectionMode(item.connectionMode)} -{" "}
                      {t("tools.loadtest.rpsLimit")}:{" "}
                      {item.rpsLimit && item.rpsLimit > 0
                        ? `${formatNumber(item.rpsLimit, 1)} (${renderRpsMode(item.rpsMode)})`
                        : t("tools.loadtest.rpsUnlimitedShort")}
                    </div>
                    <div>
                      {t("tools.loadtest.metrics.totalRequests")}:{" "}
                      {item.summary.totalRequests}
                    </div>
                    <div>
                      RPS {formatNumber(item.summary.rps, 1)} - P50{" "}
                      {formatNumber(item.summary.p50, 1)} ms - P90{" "}
                      {formatNumber(item.summary.p90, 1)} ms - P95{" "}
                      {formatNumber(item.summary.p95, 1)} ms
                    </div>
                  </HistoryCard>
                ))}
              </HistoryList>
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "center",
                  marginTop: 8,
                }}
              >
                <InlineHint>
                  {t("tools.loadtest.pagination.pageInfo", {
                    current: currentPage,
                    total: Math.max(
                      1,
                      Math.ceil(totalHistory / HISTORY_PAGE_SIZE),
                    ),
                    count: totalHistory,
                  })}
                </InlineHint>
                <div style={{ display: "flex", gap: 8 }}>
                  <StyledButton
                    variant="ghost"
                    whileTap={{ scale: 0.95 }}
                    onClick={() => fetchHistory(currentPage - 1)}
                    disabled={historyLoading || currentPage <= 1}
                  >
                    {t("tools.loadtest.pagination.prev")}
                  </StyledButton>
                  <StyledButton
                    variant="ghost"
                    whileTap={{ scale: 0.95 }}
                    onClick={() => fetchHistory(currentPage + 1)}
                    disabled={
                      historyLoading ||
                      currentPage >=
                        Math.max(1, Math.ceil(totalHistory / HISTORY_PAGE_SIZE))
                    }
                  >
                    {t("tools.loadtest.pagination.next")}
                  </StyledButton>
                </div>
              </div>
            </>
          )}
          {historyLoading && (
            <HistoryOverlay>
              <InlineHint>{t("tools.loadtest.historyLoading")}</InlineHint>
            </HistoryOverlay>
          )}
        </HistoryBody>
      </ConfigCard>
    </LoadTestContainer>
  );
};

export default LoadTest;
