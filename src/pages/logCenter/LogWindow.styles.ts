import styled from "styled-components";

export const LogWindowContainer = styled.div`
  display: flex;
  flex-direction: column;
  height: 100vh;
  gap: 14px;
  background: ${(props) => props.theme.colors.background};
  color: ${(props) => props.theme.colors.textPrimary};
`;

export const WindowHeader = styled.header`
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: ${(props) => props.theme.sizing.appHeaderHeight};
  padding: 0 16px;
  background-color: ${(props) => props.theme.colors.background};
  color: ${(props) => props.theme.colors.textPrimary};
  user-select: none;
  z-index: ${(props) => props.theme.zIndices.appHeader};
`;

export const WindowTitle = styled.div`
  font-size: ${(props) => props.theme.typography.menuFontSize};
  letter-spacing: 0.2px;
  display: flex;
  align-items: center;
  padding: 6px 4px;
`;

export const LogWindowBody = styled.div`
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 18px 20px;
  flex: 1;
  overflow: hidden;
`;

export const Header = styled.div`
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
`;

export const HeaderText = styled.div`
  display: flex;
  flex-direction: column;
  gap: 6px;
`;

export const Title = styled.h1`
  margin: 0;
  font-size: 20px;
  letter-spacing: 0.3px;
`;

export const Subtitle = styled.p`
  margin: 0;
  color: ${(props) => props.theme.colors.textSecondary};
  font-size: 13px;
`;

export const ActionRow = styled.div`
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
`;

export const FilterBar = styled.div`
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
`;

export const FilterGroup = styled.div`
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
`;

export const FilterChip = styled.button<{ $active?: boolean }>`
  border: 1px solid
    ${(props) =>
      props.$active ? props.theme.colors.primary : props.theme.colors.border};
  background: ${(props) =>
    props.$active
      ? props.theme.colors.primaryFocus
      : props.theme.colors.surface};
  color: ${(props) => props.theme.colors.textPrimary};
  border-radius: ${(props) => props.theme.radii.pill};
  padding: 4px 10px;
  font-size: 12px;
  cursor: pointer;
  transition: all 0.15s ease;

  &:hover {
    border-color: ${(props) => props.theme.colors.primary};
  }
`;

export const LogPanel = styled.div`
  flex: 1;
  border-radius: ${(props) => props.theme.radii.base};
  border: 1px solid ${(props) => props.theme.colors.border};
  background: ${(props) => props.theme.colors.surface};
  padding: 8px 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
`;

export const LogList = styled.div`
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 6px 12px 10px;
  font-family: "JetBrains Mono", "Fira Code", Menlo, monospace;
  font-size: 12px;
`;

export const LogRow = styled.div<{
  $tone?: "info" | "warn" | "error" | "debug";
  $isLast?: boolean;
}>`
  display: grid;
  grid-template-columns: 150px 90px 70px 1fr;
  gap: 10px;
  padding: 6px 4px;
  border-bottom: 1px dashed ${(props) => props.theme.colors.border};
  color: ${(props) => props.theme.colors.textPrimary};
  ${(props) => props.$isLast && "border-bottom: none;"}

  ${(props) =>
    props.$tone === "error" &&
    `
      color: ${props.theme.colors.error};
  `}

  ${(props) =>
    props.$tone === "warn" &&
    `
      color: ${props.theme.colors.warning};
  `}

  ${(props) =>
    props.$tone === "debug" &&
    `
      color: ${props.theme.colors.textHint};
  `}
`;

export const LogMeta = styled.div`
  color: ${(props) => props.theme.colors.textHint};
`;

export const LogMessage = styled.div`
  white-space: pre-wrap;
  word-break: break-word;
`;

export const LogFooter = styled.div`
  padding: 8px 12px;
  border-top: 1px solid ${(props) => props.theme.colors.border};
  display: flex;
  justify-content: space-between;
  align-items: center;
  color: ${(props) => props.theme.colors.textSecondary};
  font-size: 12px;
`;

export const EmptyState = styled.div`
  padding: 30px 12px;
  text-align: center;
  color: ${(props) => props.theme.colors.textHint};
`;
