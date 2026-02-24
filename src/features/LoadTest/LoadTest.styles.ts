import styled from "styled-components";
import { motion } from "framer-motion";
import Input from "@/components/common/Input/Input";

export const LoadTestContainer = styled.div`
  padding: calc(${(props) => props.theme.spacing.unit} * 3);
  color: ${(props) => props.theme.colors.textPrimary};
  display: flex;
  flex-direction: column;
  gap: calc(${(props) => props.theme.spacing.unit} * 2);
  background: linear-gradient(
    180deg,
    ${(props) => props.theme.colors.background},
    ${(props) => props.theme.colors.surface}
  );
  border-radius: ${(props) => props.theme.radii.base};
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.05);
`;

export const Header = styled.div`
  display: flex;
  flex-direction: column;
  gap: ${(props) => props.theme.spacing.unit};
`;

export const Title = styled.h1`
  font-size: 1.5rem;
  display: flex;
  align-items: center;
  gap: ${(props) => props.theme.spacing.unit};
`;

export const Subtitle = styled.p`
  color: ${(props) => props.theme.colors.textSecondary};
`;

export const ConfigCard = styled.div`
  background: ${(props) => props.theme.colors.surface};
  border: 1px solid ${(props) => props.theme.colors.border};
  border-radius: ${(props) => props.theme.radii.base};
  padding: calc(${(props) => props.theme.spacing.unit} * 2);
  display: flex;
  flex-direction: column;
  gap: calc(${(props) => props.theme.spacing.unit} * 1.5);
`;

export const SectionHeader = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: ${(props) => props.theme.spacing.unit};
  font-weight: 600;
`;

export const SectionTitle = styled.span`
  display: inline-flex;
  align-items: center;
  gap: ${(props) => props.theme.spacing.unit};
`;

export const SectionAction = styled.button`
  color: ${(props) => props.theme.colors.primary};
  display: inline-flex;
  align-items: center;
  gap: ${(props) => props.theme.spacing.unit};
`;

export const FormGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
  gap: calc(${(props) => props.theme.spacing.unit} * 2);
`;

export const FormRow = styled.div`
  display: flex;
  flex-direction: column;
  gap: ${(props) => props.theme.spacing.unit};
`;

export const Label = styled.label`
  font-weight: 600;
  display: flex;
  gap: ${(props) => props.theme.spacing.unit};
  align-items: center;
  color: ${(props) => props.theme.colors.textSecondary};
`;

export const TextInput = styled(Input)``;

export const Select = styled.select`
  width: 100%;
  padding: 12px 42px 12px 12px;
  border-radius: ${(props) => props.theme.radii.base};
  border: 1px solid ${(props) => props.theme.colors.border};
  background: ${(props) => props.theme.colors.background};
  color: ${(props) => props.theme.colors.textPrimary};
  appearance: none;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.04);
  background-image:
    linear-gradient(
      45deg,
      transparent 50%,
      ${(props) => props.theme.colors.textSecondary} 50%
    ),
    linear-gradient(
      135deg,
      ${(props) => props.theme.colors.textSecondary} 50%,
      transparent 50%
    ),
    linear-gradient(
      to right,
      ${(props) => props.theme.colors.border},
      ${(props) => props.theme.colors.border}
    );
  background-position:
    calc(100% - 18px) calc(50% - 2px),
    calc(100% - 12px) calc(50% - 2px),
    calc(100% - 34px) 50%;
  background-size:
    6px 6px,
    6px 6px,
    1px 60%;
  background-repeat: no-repeat;
  transition:
    border-color 0.2s ease,
    box-shadow 0.2s ease;

  &:hover:not(:disabled) {
    border-color: ${(props) => props.theme.colors.primary};
  }

  &:focus {
    border-color: ${(props) => props.theme.colors.primary};
    box-shadow: 0 0 0 3px ${(props) => props.theme.colors.primaryFocus};
    outline: none;
  }

  &:disabled {
    opacity: 0.7;
    cursor: not-allowed;
    box-shadow: none;
  }

  option {
    background: ${(props) => props.theme.colors.surface};
    color: ${(props) => props.theme.colors.textPrimary};
  }
`;

export const Textarea = styled.textarea`
  width: 100%;
  min-height: 120px;
  padding: 12px;
  border-radius: ${(props) => props.theme.radii.base};
  border: 1px solid ${(props) => props.theme.colors.border};
  background: ${(props) => props.theme.colors.background};
  color: ${(props) => props.theme.colors.textPrimary};
  line-height: 1.5;
`;

export const ControlRow = styled.div`
  display: flex;
  flex-wrap: wrap;
  gap: calc(${(props) => props.theme.spacing.unit} * 1.5);
  align-items: center;
  justify-content: space-between;
`;

export const InlineFields = styled.div`
  display: grid;
  grid-template-columns: auto 1fr;
  gap: calc(${(props) => props.theme.spacing.unit} * 1.5);
  flex: 1;
  align-items: center;
`;

export const MetricsGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: calc(${(props) => props.theme.spacing.unit} * 1.5);
`;

export const MetricCard = styled.div`
  background: ${(props) => props.theme.colors.surface};
  border: 1px solid ${(props) => props.theme.colors.border};
  border-radius: ${(props) => props.theme.radii.base};
  padding: calc(${(props) => props.theme.spacing.unit} * 1.5);
  display: grid;
  grid-template-columns: 48px 1fr;
  gap: ${(props) => props.theme.spacing.unit};
  align-items: center;
  min-height: 96px;
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.05);
`;

export const MetricIcon = styled.div<{ $color: string }>`
  width: 48px;
  height: 48px;
  border-radius: ${(props) => props.theme.radii.circle};
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: ${(props) => props.$color}22;
  color: ${(props) => props.$color};
  font-size: 1.4rem;
`;

export const MetricValue = styled.div`
  font-size: 1.2rem;
  font-weight: 700;
`;

export const MetricLabel = styled.div`
  color: ${(props) => props.theme.colors.textSecondary};
  font-size: 0.9rem;
`;

export const MetricDelta = styled.div<{ $positive?: boolean }>`
  font-size: 0.85rem;
  color: ${(props) =>
    props.$positive ? props.theme.colors.success : props.theme.colors.warning};
`;

export const ChartsGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
  gap: calc(${(props) => props.theme.spacing.unit} * 1.5);
`;

export const ChartCard = styled.div`
  background: ${(props) => props.theme.colors.surface};
  border: 1px solid ${(props) => props.theme.colors.border};
  border-radius: ${(props) => props.theme.radii.base};
  padding: calc(${(props) => props.theme.spacing.unit} * 1.5);
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.05);
`;

export const ChartHeader = styled.div`
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: ${(props) => props.theme.spacing.unit};
  color: ${(props) => props.theme.colors.textSecondary};
`;

export const ProgressBar = styled.div`
  background: ${(props) => props.theme.colors.border};
  border-radius: ${(props) => props.theme.radii.pill};
  overflow: hidden;
  height: 10px;
  width: 100%;
`;

export const ProgressFill = styled(motion.div)`
  height: 100%;
  background: linear-gradient(
    90deg,
    ${(props) => props.theme.colors.primary},
    ${(props) => props.theme.colors.secondary}
  );
`;

export const StatusRow = styled.div`
  display: flex;
  flex-direction: row;
  gap: ${(props) => props.theme.spacing.unit};
  color: ${(props) => props.theme.colors.textSecondary};
  align-items: center;
  font-size: 0.9rem;
  width: 100%;
  min-width: 0;
  justify-content: center;
  text-align: center;
`;

export const Badge = styled.span<{
  $tone?: "success" | "warning";
  $fullWidth?: boolean;
}>`
  padding: 4px 10px;
  border-radius: ${(props) => props.theme.radii.pill};
  border: 1px solid
    ${(props) =>
      props.$tone === "success"
        ? props.theme.colors.success
        : props.$tone === "warning"
          ? props.theme.colors.warning
          : props.theme.colors.border};
  color: ${(props) =>
    props.$tone === "success"
      ? props.theme.colors.success
      : props.$tone === "warning"
        ? props.theme.colors.warning
        : props.theme.colors.textSecondary};
  background: ${(props) => props.theme.colors.background};
  font-size: 0.8rem;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  width: ${(props) => (props.$fullWidth ? "100%" : "auto")};
  max-width: 100%;
  white-space: normal;
  word-break: break-word;
  text-align: left;
  flex: ${(props) => (props.$fullWidth ? "1 1 auto" : "0 1 auto")};
`;

export const InlineHint = styled.div`
  font-size: 0.85rem;
  color: ${(props) => props.theme.colors.textSecondary};
`;

export const HistoryBody = styled.div`
  position: relative;
  min-height: 360px;
`;

export const HistoryList = styled.div`
  display: flex;
  flex-direction: column;
  gap: 10px;
`;

export const HistoryCard = styled.div`
  border: 1px solid ${(props) => props.theme.colors.border};
  border-radius: 8px;
  padding: 10px 12px;
  display: grid;
  grid-template-columns: 1.4fr 1fr 1fr 1fr;
  gap: 8px;
  align-items: center;
  min-height: 64px;
  background: ${(props) => props.theme.colors.surface};
`;

export const HistoryOverlay = styled.div`
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: ${(props) => props.theme.colors.background};
  pointer-events: none;
`;

export const HeaderList = styled.div`
  display: flex;
  flex-direction: column;
  gap: ${(props) => props.theme.spacing.unit};
`;

export const HeaderRow = styled.div`
  display: grid;
  grid-template-columns: 1fr 1fr auto;
  gap: ${(props) => props.theme.spacing.unit};
  align-items: center;
`;

export const HeaderInput = styled(TextInput)`
  width: 100%;
`;
