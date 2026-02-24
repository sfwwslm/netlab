import React, { Suspense } from "react";
import { createRoot } from "react-dom/client";
import { ensureAppRootDirExists } from "@/utils/fs";
import Loading from "./components/common/Loading";
import i18next from "i18next";
import { initReactI18next } from "react-i18next";
import { getLanguage } from "@/utils/config";
import { AppRouter } from "./routes";
import InitializationError from "./components/common/InitializationError";
import * as log from "@tauri-apps/plugin-log";
import {
  isPermissionGranted,
  requestPermission,
} from "@tauri-apps/plugin-notification";

const el = document.getElementById("root")!;

// 使用 async 解决 pnpm build 时的报错
// Top-level await is not available in the configured target environment
(async () => {
  try {
    await ensureAppRootDirExists();

    await Promise.all([loadLanguageFiles(), requestNotifyPermission()]);

    await renderApp();
  } catch (error) {
    // 如果任何一步失败，则捕获错误并渲染专用的错误页面
    log.error(`应用初始化失败: ${error}`);
    if (error instanceof Error) {
      renderError(error);
    } else {
      renderError(new Error(String(error)));
    }
  }
})();

/** 加载语言文件 */
async function loadLanguageFiles() {
  // 从配置文件读取语言，如果失败或未设置，默认为 'zh'
  const savedLang = await getLanguage();

  i18next.use(initReactI18next).init({
    lng: savedLang,
    fallbackLng: "zh",
    supportedLngs: ["zh", "en", "ja"],
    debug: import.meta.env.DEV,
    interpolation: {
      escapeValue: false, // React 已经可以防范 XSS
    },
    resources: {
      zh: {
        translation: await import("./locales/zh.json"),
      },
      en: {
        translation: await import("./locales/en.json"),
      },
      ja: {
        translation: await import("./locales/ja.json"),
      },
    },
  });
}

/** 申请通知权限 */
async function requestNotifyPermission() {
  if (!(await isPermissionGranted())) {
    await requestPermission();
  }
}

/** 渲染主应用 */
async function renderApp() {
  const root = createRoot(el);

  root.render(
    <React.StrictMode>
      <Suspense fallback={<Loading />}>
        <AppRouter />
      </Suspense>
    </React.StrictMode>,
  );
}

/** 渲染错误页面 */
function renderError(error: Error) {
  const root = createRoot(el);
  root.render(
    <React.StrictMode>
      <InitializationError error={error} />
    </React.StrictMode>,
  );
}
