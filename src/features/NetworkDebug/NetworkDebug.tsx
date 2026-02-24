import React, {
  useState,
  useEffect,
  useMemo,
  useCallback,
  useRef,
} from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useTranslation } from "react-i18next";
import {
  DebugContainer,
  ConfigPanel,
  InteractionPanel,
  ConfigSection,
  SectionTitle,
  ConfigRow,
  Label,
  StyledInput,
  ActionButtons,
  SendPanel,
  LogPanel,
  SendDataHeader,
  DataFormatToggle,
  StyledTextarea,
  LogHeader,
  LogContent,
  LogEntry,
  ControlButtonContainer,
} from "./NetworkDebug.styles";
import CustomSelect from "@/components/common/CustomSelect/CustomSelect";
import { StyledButton } from "@/components/styled/StyledButton";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getFormattedTimestamp } from "@/utils/dateHelpers";
import { useModal } from "@/contexts/ModalContext";

// 定义日志条目的类型
type LogEntryType = {
  timestamp: string;
  type: "status" | "sent" | "received";
  content: string;
  source?: string;
};

// 将 HEX 字符串转换为 Uint8Array
const hexStringToUint8Array = (hexString: string): Uint8Array | null => {
  // 移除所有空格并将字母转为大写
  const cleanedHexString = hexString.replace(/\s+/g, "").toUpperCase();

  // 验证是否只包含有效的 HEX 字符 (0-9, A-F)
  if (!/^[0-9A-F]*$/.test(cleanedHexString)) {
    return null; // 包含无效字符
  }

  // 验证长度是否为偶数
  if (cleanedHexString.length % 2 !== 0) {
    return null; // 长度必须是偶数
  }

  const byteArray = new Uint8Array(cleanedHexString.length / 2);
  for (let i = 0; i < cleanedHexString.length; i += 2) {
    byteArray[i / 2] = parseInt(cleanedHexString.substring(i, i + 2), 16);
  }
  return byteArray;
};

const NetworkDebugger: React.FC = () => {
  const { t } = useTranslation();
  const { openAlert } = useModal();
  const [connectionMode, setConnectionMode] = useState("tcp_server");
  const [ipVersion, setIpVersion] = useState("ipv4");
  const [dataFormat, setDataFormat] = useState("text");

  // 分离本地和远程配置
  const [host, setHost] = useState("0.0.0.0"); // 通用主机输入，可以是本地或远程
  const [localPort, setLocalPort] = useState(8080); // 用于 TCP/UDP Server
  const [udpClientLocalPort, setUdpClientLocalPort] = useState(0); // 单独用于 UDP Client
  const [remoteHost, setRemoteHost] = useState("127.0.0.1");
  const [remotePort, setRemotePort] = useState(8080); // 用于所有 Client

  const [lineEnding, setLineEnding] = useState("none");
  const [sendText, setSendText] = useState("");
  const [autoSendEnabled, setAutoSendEnabled] = useState(false);
  const [autoSendIntervalMs, setAutoSendIntervalMs] = useState(1000);
  const [autoSendBatchSize, setAutoSendBatchSize] = useState(1);
  const [autoSendMaxSpeed, setAutoSendMaxSpeed] = useState(false);
  const [autoSending, setAutoSending] = useState(false);
  const [isConnected, setIsConnected] = useState(false);
  const logListRef = useRef<HTMLDivElement>(null);

  // 从 connectionMode 派生出协议和角色
  const { protocol, role } = useMemo(() => {
    // 确保 role 在这里被解构
    const [proto, rl] = connectionMode.split("_");
    return { protocol: proto, role: rl };
  }, [connectionMode]);

  // 日志状态
  const [logs, setLogs] = useState<LogEntryType[]>([
    {
      timestamp: getFormattedTimestamp(),
      type: "status",
      content: t("tools.networkDebug.welcome"),
    },
  ]);
  const rowVirtualizer = useVirtualizer({
    count: logs.length,
    getScrollElement: () => logListRef.current,
    estimateSize: () => 50,
    overscan: 8,
    measureElement: (element) => element.getBoundingClientRect().height,
  });

  // 监听后端事件 - 修改状态判断逻辑
  useEffect(() => {
    const unlisten = listen<LogEntryType>("network-log-event", (event) => {
      const newLog = {
        ...event.payload,
        timestamp: getFormattedTimestamp(),
      };
      // 总是添加新日志
      setLogs((prevLogs) => [...prevLogs, newLog]);

      // --- 根据角色更新 isConnected 状态 ---
      if (event.payload.type === "status") {
        const statusContent = event.payload.content;

        if (role === "server") {
          if (
            statusContent.includes("正在监听") ||
            statusContent.includes("UDP Socket 已绑定")
          ) {
            setIsConnected(true);
          } else if (
            statusContent.includes("监听失败") ||
            statusContent.includes("绑定 UDP Socket 失败")
          ) {
            setIsConnected(false);
            setAutoSending(false);
          }
          // 注意：忽略 "与 ... 的连接已断开" 消息对 isConnected 的影响
        } else {
          // --- 客户端逻辑 (TCP Client / UDP Client) ---
          if (
            statusContent.includes("成功连接到") ||
            statusContent.includes("UDP Socket 已绑定") // UDP Client 也算连接成功
          ) {
            setIsConnected(true);
          } else if (
            statusContent.includes("断开") || // 包括 "连接已断开", "手动断开连接"
            statusContent.includes("失败") || // 包括 "连接失败", "绑定 UDP Socket 失败" 等
            statusContent.includes("关闭") // 可选，如果后端发送特定关闭消息
          ) {
            setIsConnected(false);
            setAutoSending(false);
          }
        }
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [role]);

  // 自动滚动日志
  useEffect(() => {
    if (logs.length === 0) return;
    rowVirtualizer.scrollToIndex(logs.length - 1, { align: "end" });
  }, [logs.length, rowVirtualizer]);

  const connectionModeOptions = [
    { value: "tcp_client", label: t("tools.networkDebug.tcpClient") },
    { value: "tcp_server", label: t("tools.networkDebug.tcpServer") },
    { value: "udp_client", label: t("tools.networkDebug.udpClient") },
    { value: "udp_server", label: t("tools.networkDebug.udpServer") },
  ];

  const ipVersionOptions = [
    { value: "ipv4", label: "IPv4" },
    { value: "ipv6", label: "IPv6" },
  ];

  const lineEndingOptions = [
    { value: "none", label: t("tools.networkDebug.lineEndingNone") },
    { value: "lf", label: t("tools.networkDebug.lineEndingLf") },
    { value: "crlf", label: t("tools.networkDebug.lineEndingCrlf") },
  ];
  const handleClearLog = useCallback(() => {
    setLogs([
      {
        timestamp: getFormattedTimestamp(),
        type: "status",
        content: t("tools.networkDebug.logCleared"),
      },
    ]);
  }, [t]);

  const handleConnect = useCallback(async () => {
    // 根据模式决定传递哪个本地端口
    const portToBind =
      connectionMode === "udp_client" ? udpClientLocalPort : localPort;

    await invoke("connect_or_listen", {
      protocol,
      role,
      ipVersion,
      host: role === "server" ? host : remoteHost, // 根据角色传递正确的host
      localPort: Number(portToBind) || 0, // 确保空字符串或无效输入被视为0（随机端口）
      remoteHost,
      remotePort: Number(remotePort),
    });
  }, [
    connectionMode,
    udpClientLocalPort,
    localPort,
    protocol,
    role,
    ipVersion,
    host,
    remoteHost,
    remotePort,
  ]);

  const handleDisconnect = useCallback(async () => {
    await invoke("disconnect");
    setLogs((prev) => [
      ...prev,
      {
        timestamp: getFormattedTimestamp(),
        type: "status",
        content: "手动断开连接/停止监听。",
      },
    ]);
    setIsConnected(false); // 手动断开总是设置 false
    setAutoSending(false);
  }, []);

  const buildPayload = useCallback(() => {
    if (!sendText.trim()) return null;

    let dataBytes: Uint8Array | null = null;

    if (dataFormat === "hex") {
      // HEX 格式处理
      dataBytes = hexStringToUint8Array(sendText);
      if (!dataBytes) {
        openAlert({
          title: t("common.inputError"),
          message:
            "无效的 HEX 格式。请输入成对的十六进制字符 (0-9, A-F)，允许空格分隔。",
        });
        return null;
      }
      // HEX 模式下不添加结束符
    } else {
      // 文本格式处理
      let dataToSend = sendText;
      if (lineEnding === "lf") {
        dataToSend += "\n";
      } else if (lineEnding === "crlf") {
        dataToSend += "\r\n";
      }
      dataBytes = new TextEncoder().encode(dataToSend);
    }

    return dataBytes;
  }, [sendText, dataFormat, lineEnding, openAlert, t]);

  const handleSendOnce = useCallback(async () => {
    const dataBytes = buildPayload();
    if (!dataBytes) return;

    // 计算发送目标：
    // - UDP 服务端依赖后端的 last_client_addr 记忆最近的来源，此时不应覆盖为默认的 remoteHost/remotePort
    // - 其它模式继续使用用户指定的远端
    const shouldOmitTarget = protocol === "udp" && role === "server";
    const targetHost = shouldOmitTarget ? "" : remoteHost;
    const targetPort = shouldOmitTarget ? 0 : Number(remotePort);

    try {
      await invoke("send_data", {
        // 将 Uint8Array 转换为普通的 number[] 传递给 Rust
        data: Array.from(dataBytes),
        remoteHost: targetHost,
        remotePort: targetPort,
      });
      // 注意：发送成功后，日志记录在 Rust 端处理 (emit_log)，前端不再重复记录 "sent"
    } catch (error) {
      openAlert({
        title: "发送失败",
        message: `发送数据时出错: ${error}`,
      });
    }
  }, [buildPayload, remoteHost, remotePort, protocol, role, openAlert]);

  const handleToggleAutoSend = useCallback(async () => {
    if (autoSending) {
      await invoke("stop_auto_send");
      setAutoSending(false);
      return;
    }

    const dataBytes = buildPayload();
    if (!dataBytes) return;

    const shouldOmitTarget = protocol === "udp" && role === "server";
    const targetHost = shouldOmitTarget ? "" : remoteHost;
    const targetPort = shouldOmitTarget ? 0 : Number(remotePort);

    try {
      await invoke("start_auto_send", {
        data: Array.from(dataBytes),
        intervalMs: Number(autoSendIntervalMs) || 1000,
        batchSize: Number(autoSendBatchSize) || 1,
        repeat: true,
        maxSpeed: autoSendMaxSpeed,
        remoteHost: targetHost,
        remotePort: targetPort,
      });
      setAutoSending(true);
    } catch (error) {
      openAlert({
        title: "发送失败",
        message: `启动连续发送时出错: ${error}`,
      });
    }
  }, [
    autoSending,
    autoSendIntervalMs,
    autoSendBatchSize,
    autoSendMaxSpeed,
    buildPayload,
    protocol,
    role,
    remoteHost,
    remotePort,
    openAlert,
  ]);

  const handleSendAction = useCallback(async () => {
    if (autoSendEnabled) {
      await handleToggleAutoSend();
    } else {
      await handleSendOnce();
    }
  }, [autoSendEnabled, handleSendOnce, handleToggleAutoSend]);

  const getToggleButtonText = useCallback(() => {
    if (isConnected) {
      return role === "client"
        ? t("tools.networkDebug.disconnect")
        : t("tools.networkDebug.stopListening");
    } else {
      return role === "client"
        ? t("tools.networkDebug.connect")
        : t("tools.networkDebug.startListening");
    }
  }, [isConnected, role, t]);

  const handleToggleConnection = useCallback(() => {
    if (isConnected) {
      handleDisconnect();
    } else {
      handleConnect();
    }
  }, [isConnected, handleConnect, handleDisconnect]);

  return (
    <DebugContainer>
      {/* --- 左侧：配置面板 --- */}
      <ConfigPanel>
        <ConfigSection>
          <SectionTitle>
            {t("tools.networkDebug.connectionSettings")}
          </SectionTitle>
          <ConfigRow>
            <Label>{t("tools.networkDebug.mode")}</Label>
            <CustomSelect
              options={connectionModeOptions}
              value={connectionMode}
              onChange={(v) => setConnectionMode(v as string)}
            />
          </ConfigRow>
          <ConfigRow>
            <Label>{t("tools.networkDebug.ipVersion")}</Label>
            <CustomSelect
              options={ipVersionOptions}
              value={ipVersion}
              onChange={(v) => setIpVersion(v as string)}
            />
          </ConfigRow>
        </ConfigSection>

        <ConfigSection>
          <SectionTitle>{t("tools.networkDebug.targetSettings")}</SectionTitle>

          {role === "server" && (
            <>
              <ConfigRow>
                <Label>{t("tools.networkDebug.listenHost")}</Label>
                <StyledInput
                  value={host}
                  onChange={(e) => setHost(e.target.value)}
                  placeholder="0.0.0.0 或 ::"
                />
              </ConfigRow>
              <ConfigRow>
                <Label>{t("tools.networkDebug.listenPort")}</Label>
                <StyledInput
                  type="number"
                  value={localPort}
                  onChange={(e) => setLocalPort(Number(e.target.value))}
                />
              </ConfigRow>
            </>
          )}

          {role === "client" && (
            <>
              <ConfigRow>
                <Label>{t("tools.networkDebug.remoteHost")}</Label>
                <StyledInput
                  value={remoteHost}
                  onChange={(e) => setRemoteHost(e.target.value)}
                />
              </ConfigRow>
              <ConfigRow>
                <Label>{t("tools.networkDebug.remotePort")}</Label>
                <StyledInput
                  type="number"
                  value={remotePort}
                  onChange={(e) => setRemotePort(Number(e.target.value))}
                />
              </ConfigRow>
              {protocol === "udp" && (
                <ConfigRow>
                  <Label>{t("tools.networkDebug.localPortOptional")}</Label>
                  <StyledInput
                    type="number"
                    placeholder={t("tools.networkDebug.randomPort")}
                    value={udpClientLocalPort || ""} // 显示空字符串而不是0
                    onChange={(e) =>
                      setUdpClientLocalPort(Number(e.target.value))
                    }
                  />
                </ConfigRow>
              )}
            </>
          )}
        </ConfigSection>

        <ConfigSection>
          <ActionButtons style={{ gridTemplateColumns: "1fr" }}>
            <ControlButtonContainer>
              <StyledButton
                variant={isConnected ? "danger" : "primary"}
                onClick={handleToggleConnection}
              >
                {getToggleButtonText()}
              </StyledButton>
            </ControlButtonContainer>
          </ActionButtons>
        </ConfigSection>
      </ConfigPanel>

      {/* --- 右侧：交互面板 --- */}
      <InteractionPanel>
        <SendPanel>
          <SendDataHeader>
            <SectionTitle>{t("tools.networkDebug.sendData")}</SectionTitle>
            <div>
              <DataFormatToggle
                $isActive={dataFormat === "text"}
                onClick={() => setDataFormat("text")}
              >
                {t("tools.networkDebug.text")}
              </DataFormatToggle>
              <DataFormatToggle
                $isActive={dataFormat === "hex"}
                onClick={() => setDataFormat("hex")}
              >
                {t("tools.networkDebug.hex")}
              </DataFormatToggle>
            </div>
          </SendDataHeader>
          <StyledTextarea
            placeholder={
              dataFormat === "hex"
                ? "输入 HEX 数据 (例如: AA BB CC DD)" // HEX 模式下的提示
                : t("tools.networkDebug.sendPlaceholder") // 文本模式下的提示
            }
            value={sendText}
            onChange={(e) => setSendText(e.target.value)}
            // 可以添加样式根据 dataFormat 改变字体，例如等宽字体
            style={{
              fontFamily:
                dataFormat === "hex"
                  ? '"Courier New", Courier, monospace'
                  : "inherit",
            }}
          />
          <div
            style={{
              display: "flex",
              gap: "0.5rem",
              justifyContent: "space-between",
              alignItems: "center",
              marginTop: "0.5rem",
              flexWrap: "wrap",
            }}
          >
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: "0.5rem",
                flexWrap: "wrap",
                minWidth: 0,
              }}
            >
              <Label style={{ margin: 0, whiteSpace: "nowrap" }}>
                {t("tools.networkDebug.appendTerminator")}
              </Label>
              <div
                style={{ display: "flex", gap: "0.35rem", flexWrap: "wrap" }}
              >
                {lineEndingOptions.map((opt) => (
                  <DataFormatToggle
                    key={opt.value}
                    $isActive={lineEnding === opt.value}
                    onClick={() => setLineEnding(opt.value as string)}
                    style={{ marginLeft: 0 }}
                  >
                    {opt.label}
                  </DataFormatToggle>
                ))}
              </div>
            </div>
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: "0.5rem",
                flexWrap: "wrap",
              }}
            >
              <Label style={{ margin: 0, whiteSpace: "nowrap" }}>
                {t("tools.networkDebug.continuousSend")}
              </Label>
              <DataFormatToggle
                $isActive={autoSendEnabled}
                onClick={() => {
                  if (autoSending) {
                    invoke("stop_auto_send");
                    setAutoSending(false);
                  }
                  setAutoSendEnabled((prev) => !prev);
                }}
                style={{ marginLeft: 0 }}
              >
                {autoSendEnabled
                  ? t("tools.networkDebug.continuousOn")
                  : t("tools.networkDebug.continuousOff")}
              </DataFormatToggle>
              <Label style={{ margin: 0, whiteSpace: "nowrap" }}>
                {t("tools.networkDebug.intervalMs")}
              </Label>
              <StyledInput
                type="number"
                value={autoSendIntervalMs}
                onChange={(e) => setAutoSendIntervalMs(Number(e.target.value))}
                disabled={!autoSendEnabled || autoSendMaxSpeed}
                style={{ width: "120px" }}
              />
              <Label style={{ margin: 0, whiteSpace: "nowrap" }}>
                {t("tools.networkDebug.batchSize")}
              </Label>
              <StyledInput
                type="number"
                value={autoSendBatchSize}
                onChange={(e) => setAutoSendBatchSize(Number(e.target.value))}
                disabled={!autoSendEnabled || autoSendMaxSpeed}
                style={{ width: "120px" }}
              />
              <DataFormatToggle
                $isActive={autoSendMaxSpeed}
                onClick={() => setAutoSendMaxSpeed((prev) => !prev)}
                style={{ marginLeft: 0 }}
                disabled={!autoSendEnabled}
              >
                {t("tools.networkDebug.maxSpeed")}
              </DataFormatToggle>
            </div>
            <StyledButton
              variant="primary"
              style={{ flexShrink: 0 }}
              onClick={handleSendAction}
              disabled={!isConnected}
            >
              {autoSendEnabled
                ? autoSending
                  ? t("tools.networkDebug.stopAutoSend")
                  : t("tools.networkDebug.startAutoSend")
                : t("tools.networkDebug.send")}
            </StyledButton>
          </div>
        </SendPanel>
        <LogPanel>
          <LogHeader>
            <SectionTitle>{t("tools.networkDebug.log")}</SectionTitle>
            <StyledButton variant="ghost" onClick={handleClearLog}>
              {t("tools.networkDebug.clearLog")}
            </StyledButton>
          </LogHeader>
          <LogContent ref={logListRef}>
            <div
              style={{
                height: rowVirtualizer.getTotalSize(),
                position: "relative",
              }}
            >
              {rowVirtualizer.getVirtualItems().map((virtualRow) => {
                const entry = logs[virtualRow.index];
                if (!entry) return null;
                const meta = `[${entry.timestamp}] [${entry.type.toUpperCase()}${
                  entry.source ? ` FROM ${entry.source}` : ""
                }]`;
                const isStatus = entry.type === "status";
                const arrow = entry.type === "sent" ? ">>>" : "<<<";
                return (
                  <div
                    key={`${virtualRow.index}-${entry.timestamp}`}
                    ref={rowVirtualizer.measureElement}
                    data-index={virtualRow.index}
                    style={{
                      position: "absolute",
                      top: 0,
                      left: 0,
                      width: "100%",
                      transform: `translateY(${virtualRow.start}px)`,
                    }}
                  >
                    {isStatus ? (
                      <LogEntry type={entry.type}>
                        {meta} {entry.content}
                      </LogEntry>
                    ) : (
                      <>
                        <div className="log-entry__meta">
                          {meta} {arrow}
                        </div>
                        <LogEntry
                          type={entry.type}
                          className="log-entry__content"
                        >
                          {entry.content}
                        </LogEntry>
                      </>
                    )}
                  </div>
                );
              })}
            </div>
          </LogContent>
        </LogPanel>
      </InteractionPanel>
    </DebugContainer>
  );
};

export default NetworkDebugger;
