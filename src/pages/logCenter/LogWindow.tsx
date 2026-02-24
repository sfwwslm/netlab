import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { GlobalStyle } from "@/styles/GlobalStyles";
import { AppProvider } from "@/contexts/AppProvider";
import {
  ActionRow,
  EmptyState,
  FilterBar,
  FilterChip,
  FilterGroup,
  LogFooter,
  LogList,
  LogMessage,
  LogMeta,
  LogPanel,
  LogRow,
  LogWindowBody,
  LogWindowContainer,
  WindowHeader,
  WindowTitle,
} from "./LogWindow.styles";
import { StyledButton } from "@/components/styled/StyledButton";
import styled from "styled-components";
import WindowControls from "@/components/layout/WindowControls";
import dragging from "@/utils/dragging";
import { useDisableBrowserShortcuts } from "@/hooks/useDisableBrowserShortcuts";

const ActionButton = styled(StyledButton)`
  min-width: 72px;
  height: 34px;
  padding: 0 14px;
`;

type LogEntry = {
  id: number;
  timestampMs: number;
  scope: string;
  level: "info" | "warn" | "error" | "debug";
  message: string;
  source?: string | null;
};

const scopeOrder = ["loadtest", "network", "proxy", "socks5", "system"];

const LogWindowContent: React.FC = () => {
  const { t, i18n } = useTranslation();
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [activeScopes, setActiveScopes] = useState<Record<string, boolean>>({
    loadtest: true,
    network: true,
    proxy: true,
    socks5: true,
    system: true,
  });
  const [autoScrollEnabled, setAutoScrollEnabled] = useState(true);
  const listRef = useRef<HTMLDivElement>(null);

  const loadEntries = useCallback(async () => {
    const data = await invoke<LogEntry[]>("list_app_logs");
    setEntries(data);
  }, []);

  useEffect(() => {
    loadEntries();
  }, [loadEntries]);

  useEffect(() => {
    const unlistenPromise = listen<LogEntry>("app-log:entry", (event) => {
      setEntries((prev) => {
        const next = [...prev, event.payload];
        return next.length > 2000 ? next.slice(next.length - 2000) : next;
      });
    });
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    const unlistenPromise = listen<{ language: string }>(
      "app:language-changed",
      (event) => {
        if (event.payload?.language) {
          i18n.changeLanguage(event.payload.language);
        }
      },
    );
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [i18n]);

  useEffect(() => {
    const window = getCurrentWindow();
    window.setTitle(t("logWindow.title"));
  }, [i18n.language, t]);

  useDisableBrowserShortcuts();

  const scopes = useMemo(
    () =>
      scopeOrder.map((scope) => ({
        key: scope,
        label: t(`logWindow.scopes.${scope}`),
      })),
    [t],
  );
  const scopeLabels = useMemo(
    () =>
      scopes.reduce<Record<string, string>>((acc, item) => {
        acc[item.key] = item.label;
        return acc;
      }, {}),
    [scopes],
  );

  const formatTime = useCallback(
    (timestampMs: number) => {
      const date = new Date(timestampMs);
      const datePart = date.toLocaleDateString(i18n.language);
      const timePart = date.toLocaleTimeString(i18n.language, {
        hour12: false,
      });
      const ms = String(date.getMilliseconds()).padStart(3, "0");
      return `${datePart} ${timePart}.${ms}`;
    },
    [i18n.language],
  );

  const filteredEntries = useMemo(
    () => entries.filter((entry) => activeScopes[entry.scope] ?? true),
    [entries, activeScopes],
  );

  const rowVirtualizer = useVirtualizer({
    count: filteredEntries.length,
    getScrollElement: () => listRef.current,
    estimateSize: () => 40,
    overscan: 8,
    measureElement: (element) => element.getBoundingClientRect().height,
  });

  useEffect(() => {
    if (!autoScrollEnabled) return;
    if (filteredEntries.length === 0) return;
    rowVirtualizer.scrollToIndex(filteredEntries.length - 1, {
      align: "end",
    });
  }, [autoScrollEnabled, filteredEntries.length, rowVirtualizer]);

  const handleToggleScope = useCallback((scope: string) => {
    setActiveScopes((prev) => ({ ...prev, [scope]: !prev[scope] }));
  }, []);

  const handleCopy = useCallback(async () => {
    if (filteredEntries.length === 0) return;
    const content = filteredEntries
      .map((entry) => {
        const source = entry.source ? ` (${entry.source})` : "";
        return `[${formatTime(entry.timestampMs)}] [${
          entry.scope
        }] [${entry.level}] ${entry.message}${source}`;
      })
      .join("\n");
    await writeText(content);
  }, [filteredEntries, formatTime]);

  const handleExport = useCallback(async () => {
    if (filteredEntries.length === 0) return;
    const defaultPath = `netlab-logs-${Date.now()}.json`;
    const targetPath = await save({
      defaultPath,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!targetPath) return;
    await writeTextFile(targetPath, JSON.stringify(filteredEntries, null, 2));
  }, [filteredEntries]);

  const handleClear = useCallback(async () => {
    await invoke("clear_app_logs");
    setEntries([]);
  }, []);

  const handleToggleAutoScroll = useCallback(() => {
    setAutoScrollEnabled((prev) => !prev);
  }, []);

  return (
    <LogWindowContainer>
      <WindowHeader onMouseDown={dragging}>
        <WindowTitle>{t("logWindow.title")}</WindowTitle>
        <WindowControls />
      </WindowHeader>
      <LogWindowBody>
        <FilterBar>
          <FilterGroup>
            {scopes.map((scope) => (
              <FilterChip
                key={scope.key}
                $active={activeScopes[scope.key]}
                onClick={() => handleToggleScope(scope.key)}
              >
                {scope.label}
              </FilterChip>
            ))}
          </FilterGroup>
          <ActionRow>
            <ActionButton
              variant="ghost"
              whileTap={{ scale: 0.96 }}
              onClick={handleToggleAutoScroll}
            >
              {autoScrollEnabled
                ? t("logWindow.actions.pauseScroll")
                : t("logWindow.actions.autoScroll")}
            </ActionButton>
            <ActionButton
              variant="ghost"
              whileTap={{ scale: 0.96 }}
              onClick={handleCopy}
            >
              {t("logWindow.actions.copy")}
            </ActionButton>
            <ActionButton
              variant="ghost"
              whileTap={{ scale: 0.96 }}
              onClick={handleExport}
            >
              {t("logWindow.actions.export")}
            </ActionButton>
            <ActionButton
              variant="danger"
              whileTap={{ scale: 0.96 }}
              onClick={handleClear}
            >
              {t("logWindow.actions.clear")}
            </ActionButton>
          </ActionRow>
        </FilterBar>

        <LogPanel>
          <LogList ref={listRef}>
            {filteredEntries.length === 0 ? (
              <EmptyState>{t("logWindow.empty")}</EmptyState>
            ) : (
              <div
                style={{
                  height: rowVirtualizer.getTotalSize(),
                  position: "relative",
                }}
              >
                {rowVirtualizer.getVirtualItems().map((virtualRow) => {
                  const entry = filteredEntries[virtualRow.index];
                  if (!entry) return null;
                  return (
                    <LogRow
                      key={entry.id}
                      ref={rowVirtualizer.measureElement}
                      data-index={virtualRow.index}
                      $tone={entry.level}
                      $isLast={virtualRow.index === filteredEntries.length - 1}
                      style={{
                        position: "absolute",
                        top: 0,
                        left: 0,
                        width: "100%",
                        transform: `translateY(${virtualRow.start}px)`,
                      }}
                    >
                      <LogMeta>{formatTime(entry.timestampMs)}</LogMeta>
                      <LogMeta>
                        {scopeLabels[entry.scope] ?? entry.scope}
                      </LogMeta>
                      <LogMeta>{entry.level}</LogMeta>
                      <LogMessage>
                        {entry.message}
                        {entry.source ? ` (${entry.source})` : ""}
                      </LogMessage>
                    </LogRow>
                  );
                })}
              </div>
            )}
          </LogList>
          <LogFooter>
            <div>{t("logWindow.count", { count: filteredEntries.length })}</div>
            <div>{t("logWindow.buffer", { count: entries.length })}</div>
          </LogFooter>
        </LogPanel>
      </LogWindowBody>
    </LogWindowContainer>
  );
};

const LogWindow: React.FC = () => (
  <AppProvider>
    <GlobalStyle />
    <LogWindowContent />
  </AppProvider>
);

export default LogWindow;
