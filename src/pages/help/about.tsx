import { useState, useEffect, useMemo } from "react";
import { getVersion, getTauriVersion } from "@tauri-apps/api/app";
import styled from "styled-components";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useTranslation } from "react-i18next";
import Tooltip from "@/components/common/Tooltip/Tooltip";
import { parseChangelogTree } from "@/utils";

const AboutContainer = styled.div`
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  box-sizing: border-box;
  background-color: transparent;
  color: ${(props) => props.theme.colors.textPrimary};
`;

const Content = styled.div`
  text-align: center;
  max-width: 80%;
  margin-top: -20vh;
  color: ${(props) => props.theme.colors.textPrimary};

  p {
    font-weight: bold;
    margin-top: 0.5rem;
    margin-bottom: 0.5rem;
  }
`;

const Version = styled.div`
  position: absolute;
  right: 3rem;
  bottom: 1.5rem;
  font-size: 0.9rem;
  font-weight: bold;
  text-align: left;
  color: ${(props) => props.theme.colors.textPrimary};

  p {
    cursor: pointer;
    &:hover {
      color: ${(props) => props.theme.colors.primary};
    }
  }
`;

const Paragraph = styled.p`
  font-size: 1rem;
  font-weight: bold;
  margin-bottom: 1rem;
`;

/**
 * 关于页面
 * @description 显示应用、框架和构建版本等信息
 */
export default function About() {
  const [appVersion, setAppVersion] = useState("...");
  const [tauriVersion, setTauriVersion] = useState("...");
  const { t } = useTranslation();

  /**
   * 提取版本号，移除前缀（如~, ^）
   * @param version - 原始版本字符串
   */
  function extractVersion(version: string) {
    return version.replace(/^[~^]/, "");
  }

  /**
   * 从 __CHANGELOG_CONTENT__ 中提取版本号
   */
  const changelogVersion = useMemo(() => {
    const sections = parseChangelogTree(__CHANGELOG_CONTENT__);
    if (sections.length > 0) {
      return `${sections[0].title}:`;
    }
    return "";
  }, []);

  useEffect(() => {
    getVersion().then(setAppVersion);
    getTauriVersion().then(setTauriVersion);
  }, []);

  return (
    <AboutContainer className="about-container">
      <Content>
        <Paragraph>{t("help.about.tagline")}</Paragraph>
      </Content>
      <Version className="version" style={{ display: "grid" }}>
        <Tooltip text={`${changelogVersion}${__GIT_HASH__}`}>
          {/* 为了避免 Tooltip 影响 p 标签的 hover 效果，将 p 标签的样式移到 style 属性中 */}
          <p style={{ cursor: "default", color: "inherit" }}>
            Version: {appVersion}
          </p>
        </Tooltip>

        <Tooltip text={t("help.about.openTauriWebsite")}>
          <p onClick={async () => await openUrl(t("help.about.tauriUrl"))}>
            Tauri: {tauriVersion}
          </p>
        </Tooltip>

        <Tooltip text={t("help.about.openReactWebsite")}>
          <p onClick={async () => await openUrl(t("help.about.reactUrl"))}>
            React: {extractVersion(__REACT_VERSION__)}
          </p>
        </Tooltip>

        <Tooltip text={t("help.about.openViteWebsite")}>
          <p onClick={async () => await openUrl(t("help.about.viteUrl"))}>
            Vite: {__VITE_VERSION__}
          </p>
        </Tooltip>
      </Version>
    </AboutContainer>
  );
}
