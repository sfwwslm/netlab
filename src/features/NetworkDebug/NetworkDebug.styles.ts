import styled from "styled-components";
import Input from "@/components/common/Input/Input";

export const DebugContainer = styled.div`
  display: grid;
  grid-template-columns: minmax(260px, 320px) 1fr; /* 左侧随窗口缩放，避免溢出 */
  height: 100%;
  min-height: 0; /* 防止网格项撑开导致外层出现滚动条 */
  padding: 1rem;
  gap: 1rem;
  background-color: ${(props) => props.theme.colors.background};
  color: ${(props) => props.theme.colors.textPrimary};
  overflow: hidden; /* 防止外层容器出现滚动条 */
`;

// --- 左侧配置面板 ---
export const ConfigPanel = styled.div`
  display: flex;
  flex-direction: column;
  gap: 1rem;
  padding: 1rem;
  border: 1px solid ${(props) => props.theme.colors.border};
  border-radius: ${(props) => props.theme.radii.base};
  background-color: ${(props) => props.theme.colors.surface};
  overflow: visible; /* 允许下拉面板溢出显示 */
`;

export const ConfigSection = styled.div`
  display: flex;
  flex-direction: column;
  gap: 1rem;
`;

export const SectionTitle = styled.h3`
  font-size: 1.1rem;
  font-weight: bold;
  color: ${(props) => props.theme.colors.textPrimary};
  margin: 0;
  padding-bottom: 0.5rem;
  border-bottom: 1px solid ${(props) => props.theme.colors.border};
`;

export const ConfigRow = styled.div`
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
`;

export const Label = styled.label`
  font-size: 0.9rem;
  color: ${(props) => props.theme.colors.textSecondary};
`;

export const StyledInput = styled(Input)``;

export const ActionButtons = styled.div`
  margin-top: 0; /* 避免按钮过低导致遮挡 */
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.5rem;
`;

// --- 右侧交互面板 ---
export const InteractionPanel = styled.div`
  display: grid;
  grid-template-rows: auto 1fr; /* 发送区高度自适应，日志区填满剩余空间 */
  gap: 1rem;
  min-height: 0; /* 防止 flex/grid item 溢出 */
`;

export const SendPanel = styled.div`
  display: flex;
  flex-direction: column;
  padding: 1rem;
  border: 1px solid ${(props) => props.theme.colors.border};
  border-radius: ${(props) => props.theme.radii.base};
  background-color: ${(props) => props.theme.colors.surface};
`;

export const SendDataHeader = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1rem;
`;

export const DataFormatToggle = styled.button<{ $isActive: boolean }>`
  padding: 0.3rem 0.8rem;
  font-size: 0.8rem;
  color: ${(props) =>
    props.$isActive
      ? props.theme.colors.textOnPrimary
      : props.theme.colors.textSecondary};
  background-color: ${(props) =>
    props.$isActive ? props.theme.colors.primary : "transparent"};
  border: 1px solid
    ${(props) =>
      props.$isActive ? props.theme.colors.primary : props.theme.colors.border};
  border-radius: ${(props) => props.theme.radii.pill};
  margin-left: 0.5rem;
`;

export const StyledTextarea = styled.textarea`
  min-height: 80px;
  padding: 0.5rem;
  border: 1px solid ${(props) => props.theme.colors.border};
  border-radius: ${(props) => props.theme.radii.base};
  background-color: ${(props) => props.theme.colors.background};
  color: ${(props) => props.theme.colors.textPrimary};
  resize: vertical;
`;

export const LogPanel = styled.div`
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 1rem;
  border: 1px solid ${(props) => props.theme.colors.border};
  border-radius: ${(props) => props.theme.radii.base};
  background-color: ${(props) => props.theme.colors.surface};
`;

export const LogHeader = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1rem;
  flex-shrink: 0;
`;

export const LogContent = styled.div`
  flex-grow: 1;
  overflow-y: auto;
  background-color: ${(props) => props.theme.colors.background};
  border-radius: ${(props) => props.theme.radii.base};
  padding: 1rem;
  font-family: "Courier New", Courier, monospace;
  font-size: 0.9rem;
  white-space: pre-wrap;
  word-break: break-all;
`;

export const LogEntry = styled.div<{ type: "status" | "sent" | "received" }>`
  margin-bottom: 0.5rem;
  color: ${(props) => {
    switch (props.type) {
      case "sent":
        return props.theme.colors.primary;
      case "received":
        return props.theme.colors.success;
      case "status":
      default:
        return props.theme.colors.textSecondary;
    }
  }};
`;

export const ControlButtonContainer = styled.div`
  display: flex;
  align-items: center; // 垂直居中图标和按钮
  justify-content: center; // 水平居中内容
  gap: 8px; // 在图标和按钮之间添加间距
  width: 100%; // 占据 ActionButtons 的宽度
`;
