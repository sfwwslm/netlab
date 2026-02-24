import React from "react";
import styled from "styled-components";
import { GlobalStyle } from "@/styles/GlobalStyles";
import { darkTheme } from "@/styles/themes";
import { ThemeProvider } from "styled-components";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { appConfigDir } from "@tauri-apps/api/path";

const ErrorContainer = styled.div`
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  height: 100vh;
  padding: 2rem;
  box-sizing: border-box;
  text-align: center;
  background-color: ${darkTheme.colors.background};
  color: ${darkTheme.colors.textPrimary};
`;

const ErrorTitle = styled.h1`
  font-size: 2rem;
  color: ${darkTheme.colors.error};
  margin-bottom: 1rem;
`;

const ErrorMessage = styled.p`
  font-size: 1.1rem;
  color: ${darkTheme.colors.textSecondary};
  max-width: 600px;
  line-height: 1.6;
`;

const CodeBlock = styled.code`
  background-color: ${darkTheme.colors.surface};
  border: 1px solid ${darkTheme.colors.border};
  padding: 0.5rem 1rem;
  border-radius: 6px;
  margin: 1rem 0;
  display: inline-block;
  color: ${darkTheme.colors.textPrimary};
  font-family: "Courier New", Courier, monospace;
`;

/** 退出按钮的样式 */
const ExitButton = styled.button`
  margin-top: 2.5rem;
  padding: 12px 28px;
  border: 1px solid ${darkTheme.colors.border};
  border-radius: 8px;
  background-color: ${darkTheme.colors.surface};
  color: ${darkTheme.colors.textPrimary};
  font-size: 1rem;
  font-weight: bold;
  cursor: pointer;
  transition: all 0.2s ease-in-out;

  &:hover {
    background-color: ${darkTheme.colors.error};
    color: white;
    border-color: ${darkTheme.colors.error};
  }
`;

interface InitializationErrorProps {
  error: Error;
}

const InitializationError: React.FC<InitializationErrorProps> = ({ error }) => {
  const isMigrationError = error.message.includes("migration");

  //** 定义处理退出的函数 */
  const handleExit = async () => {
    await getCurrentWindow().close();
  };

  return (
    <ThemeProvider theme={darkTheme}>
      <GlobalStyle />
      <ErrorContainer>
        <ErrorTitle>应用启动失败</ErrorTitle>
        <ErrorMessage>
          启动过程遇到了一个无法恢复的严重错误。请尝试以下解决方案后重启应用。
        </ErrorMessage>

        {isMigrationError && (
          <>
            <ErrorMessage>
              <br />
              最简单的解决方法是删除旧的配置文件，应用将在下次启动时自动重建。
              <br />
            </ErrorMessage>
            <CodeBlock>{appConfigDir()}</CodeBlock>
          </>
        )}

        <details style={{ marginTop: "2rem", width: "100%" }}>
          <summary
            style={{
              cursor: "pointer",
              color: darkTheme.colors.textSecondary,
            }}
          >
            点击查看详细错误信息
          </summary>
          <CodeBlock
            style={{
              maxWidth: "80vw",
              overflow: "auto",
              textAlign: "left",
              marginTop: "1rem",
            }}
          >
            {error.message}
          </CodeBlock>
        </details>

        <ExitButton onClick={handleExit}>退出应用</ExitButton>
      </ErrorContainer>
    </ThemeProvider>
  );
};

export default InitializationError;
