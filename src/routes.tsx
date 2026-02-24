import React, { Suspense, lazy } from "react";
import { createBrowserRouter, RouterProvider } from "react-router-dom";
import App from "./pages/_app";
import Loading from "./components/common/Loading";

// 使用懒加载提高性能
const Home = lazy(() => import("./pages/index"));
const BytesConverterPage = lazy(() => import("./pages/tools/bytes"));
const LoadTestPage = lazy(() => import("./pages/tools/loadtest"));
const NetworkDebugPage = lazy(() => import("./pages/tools/network"));
const ProxyPage = lazy(() => import("./pages/tools/proxy"));
const AboutPage = lazy(() => import("./pages/help/about"));
const ChangelogPage = lazy(() => import("./pages/help/changelog"));
const CheckUpdatePage = lazy(() => import("./pages/help/CheckUpdate"));
const LogWindowPage = lazy(() => import("./pages/logCenter/LogWindow"));

// 创建路由配置
const router = createBrowserRouter([
  {
    path: "/logs",
    element: (
      <Suspense fallback={<Loading />}>
        <LogWindowPage />
      </Suspense>
    ),
  },
  {
    path: "/",
    element: <App />,
    children: [
      {
        index: true,
        element: (
          <Suspense fallback={<Loading />}>
            <Home />
          </Suspense>
        ),
      },
      {
        path: "tools/bytes",
        element: (
          <Suspense fallback={<Loading />}>
            <BytesConverterPage />
          </Suspense>
        ),
      },
      {
        path: "tools/loadtest",
        element: (
          <Suspense fallback={<Loading />}>
            <LoadTestPage />
          </Suspense>
        ),
      },
      {
        path: "tools/network",
        element: (
          <Suspense fallback={<Loading />}>
            <NetworkDebugPage />
          </Suspense>
        ),
      },
      {
        path: "tools/proxy",
        element: (
          <Suspense fallback={<Loading />}>
            <ProxyPage />
          </Suspense>
        ),
      },
      {
        path: "help/about",
        element: (
          <Suspense fallback={<Loading />}>
            <AboutPage />
          </Suspense>
        ),
      },
      {
        path: "help/changelog",
        element: (
          <Suspense fallback={<Loading />}>
            <ChangelogPage />
          </Suspense>
        ),
      },
      {
        path: "help/CheckUpdate",
        element: (
          <Suspense fallback={<Loading />}>
            <CheckUpdatePage />
          </Suspense>
        ),
      },
    ],
  },
]);

// 路由提供者组件
export const AppRouter: React.FC = () => {
  return <RouterProvider router={router} />;
};
