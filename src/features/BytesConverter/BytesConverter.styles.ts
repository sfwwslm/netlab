import styled, { keyframes } from "styled-components";
import { motion } from "framer-motion";
import { Theme } from "@/styles/themes";
import InputBase from "@/components/common/Input/Input";

/**
 * 霓虹灯闪烁动画
 */
const neonGlow = keyframes`
  0%, 100% {
    text-shadow: 0 0 3px rgba(0, 188, 212, 0.7), 0 0 6px rgba(0, 188, 212, 0.5);
    box-shadow: 0 0 5px rgba(0, 188, 212, 0.4), 0 0 8px rgba(0, 188, 212, 0.3);
  }
  50% {
    text-shadow: 0 0 8px rgba(0, 188, 212, 0.9), 0 0 15px rgba(0, 188, 212, 0.7), 0 0 25px rgba(0, 188, 212, 0.5);
    box-shadow: 0 0 10px rgba(0, 188, 212, 0.6), 0 0 18px rgba(0, 188, 212, 0.5), 0 0 30px rgba(0, 188, 212, 0.4);
  }
`;

export const ConverterContainer = styled(motion.div)<{ theme: Theme }>`
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: calc(${(props) => props.theme.spacing.unit} * 4);
  background-color: ${(props) => props.theme.colors.background};
  min-height: 100%;
  box-sizing: border-box;
`;

export const Title = styled(motion.h1)<{ theme: Theme }>`
  color: ${(props) => props.theme.colors.primary};
  font-size: 2rem;
  margin-bottom: calc(${(props) => props.theme.spacing.unit} * 3);
  letter-spacing: 1.5px;
  animation: ${neonGlow} 3s infinite alternate;
  text-transform: uppercase;
  padding: calc(${(props) => props.theme.spacing.unit} * 1.5)
    calc(${(props) => props.theme.spacing.unit} * 3);
  border: 1px solid ${(props) => props.theme.colors.primary};
  border-radius: ${(props) => props.theme.radii.base};
  background-color: rgba(0, 188, 212, 0.05);
  display: inline-block;
  font-weight: unset;
`;

export const Description = styled.p<{ theme: Theme }>`
  color: ${(props) => props.theme.colors.textSecondary};
  font-size: 1.1rem;
  font-weight: normal;
  text-align: center;
  max-width: 600px;
  margin-bottom: calc(${(props) => props.theme.spacing.unit} * 4);
  line-height: 1.6;
  letter-spacing: normal;
  text-shadow: none;
`;

export const FormGrid = styled(motion.div)<{ theme: Theme }>`
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  gap: calc(${(props) => props.theme.spacing.unit} * 3);
  padding: calc(${(props) => props.theme.spacing.unit} * 4);
  background-color: ${(props) => props.theme.colors.surface};
  border-radius: ${(props) => props.theme.radii.base};
  box-shadow: 0 8px 30px rgba(0, 0, 0, 0.5);
  width: 100%;
  max-width: 900px;
  border: 1px solid ${(props) => props.theme.colors.border};
  position: relative;
`;

export const FormGroup = styled.div`
  display: flex;
  flex-direction: column;
  position: relative;
`;

export const Label = styled.label<{ theme: Theme }>`
  color: ${(props) => props.theme.colors.textSecondary};
  font-size: 0.95rem;
  margin-bottom: calc(${(props) => props.theme.spacing.unit} / 2);
  display: flex;
  align-items: center;

  svg {
    margin-right: calc(${(props) => props.theme.spacing.unit} / 2);
    color: ${(props) => props.theme.colors.primary};
  }
`;

export const Input = styled(InputBase)<{ theme: Theme }>`
  width: 100%;
  padding: calc(${(props) => props.theme.spacing.unit} * 1.5);
  border: 1px solid ${(props) => props.theme.colors.border};
  border-radius: ${(props) => props.theme.radii.base};
  background-color: ${(props) => props.theme.colors.background};
  color: ${(props) => props.theme.colors.textPrimary};
  font-size: 1rem;
  transition:
    border-color 0.3s ease,
    box-shadow 0.3s ease;

  &:focus {
    border-color: ${(props) => props.theme.colors.primary};
    box-shadow: 0 0 0 3px ${(props) => props.theme.colors.primaryFocus};
    background-color: ${(props) =>
      props.theme.colors.background}; /* 保持一致 */
  }
`;

export const CopyButton = styled.button<{ theme: Theme }>`
  position: absolute;
  right: calc(${(props) => props.theme.spacing.unit} * 1);
  top: 50%;
  transform: translateY(calc(-50% + ${(props) => props.theme.spacing.unit}));
  background: none;
  border: none;
  color: ${(props) => props.theme.colors.textSecondary};
  cursor: pointer;
  font-size: 1.1rem;
  padding: calc(${(props) => props.theme.spacing.unit} / 2);
  border-radius: ${(props) => props.theme.radii.base};
  transition: all 0.2s ease;

  &:hover {
    color: ${(props) => props.theme.colors.primary};
    background-color: rgba(0, 188, 212, 0.1);
  }
`;

export const UnitSwitchContainer = styled.div<{ theme: Theme }>`
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  align-items: center;
  gap: calc(${(props) => props.theme.spacing.unit} * 2);
  margin-bottom: calc(${(props) => props.theme.spacing.unit} * 3);
  margin-top: calc(${(props) => props.theme.spacing.unit} * -1);
  width: 100%;
  max-width: 900px;
  box-sizing: border-box;
  padding: 0 calc(${(props) => props.theme.spacing.unit} * 4);
  position: relative;
`;

export const UnitSwitchButton = styled(motion.button)<{
  $isActive: boolean;
  theme: Theme;
}>`
  border: 1px solid ${(props) => props.theme.colors.primary};
  padding: calc(${(props) => props.theme.spacing.unit} * 1)
    calc(${(props) => props.theme.spacing.unit} * 2);
  border-radius: 20px;
  font-size: 0.9rem;
  font-weight: bold;
  cursor: pointer;
  transition: all 0.3s ease;

  /* 根据 $isActive 状态来改变颜色 */
  background-color: ${(props) =>
    props.$isActive ? props.theme.colors.primary : "transparent"};
  color: ${(props) =>
    props.$isActive
      ? props.theme.colors.background
      : props.theme.colors.textPrimary};

  &:hover {
    background-color: ${(props) => props.theme.colors.primary};
    color: ${(props) => props.theme.colors.background};
    box-shadow: 0 2px 10px rgba(0, 188, 212, 0.4);
  }
`;

export const ResetButtonContainer = styled.div`
  position: absolute;
  top: calc(${(props) => props.theme.spacing.unit} * 1.5);
  right: calc(${(props) => props.theme.spacing.unit} * 2.5);
  z-index: ${(props) => props.theme.zIndices.sticky};

  svg {
    height: ${(props) => props.theme.iconButton.size.medium};
    width: ${(props) => props.theme.iconButton.size.medium};
    color: ${(props) => props.theme.colors.primary};

    &:active {
      transform: translateY(0) scale(0.95);
    }
  }
`;

export const ModeSwitchContainer = styled.div<{ theme: Theme }>`
  display: flex;
  justify-content: center;
  gap: calc(${(props) => props.theme.spacing.unit} * 2);
  margin-bottom: calc(${(props) => props.theme.spacing.unit} * 4);
  margin-top: calc(${(props) => props.theme.spacing.unit} * -1);
`;

export const ModeSwitchButton = styled(UnitSwitchButton)``;
