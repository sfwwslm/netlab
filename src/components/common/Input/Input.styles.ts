import styled from "styled-components";

const InputBase = styled.input`
  width: 100%;
  padding: 12px;
  border-radius: ${(props) => props.theme.radii.base};
  border: 1px solid ${(props) => props.theme.colors.border};
  background: ${(props) => props.theme.colors.background};
  color: ${(props) => props.theme.colors.textPrimary};
  transition:
    border-color 0.2s ease,
    box-shadow 0.2s ease;
  box-sizing: border-box;
  font: inherit;

  &:focus {
    border-color: ${(props) => props.theme.colors.primary};
    box-shadow: 0 0 0 3px ${(props) => props.theme.colors.primaryFocus};
    outline: none;
  }

  &:disabled {
    opacity: 0.7;
    cursor: not-allowed;
  }

  &:-webkit-autofill,
  &:-webkit-autofill:hover,
  &:-webkit-autofill:focus,
  &:-webkit-autofill:active {
    -webkit-box-shadow: 0 0 0 1000px ${(props) => props.theme.colors.background}
      inset;
    -webkit-text-fill-color: ${(props) => props.theme.colors.textPrimary};
    transition: background-color 5000s ease-in-out 0s;
    caret-color: ${(props) => props.theme.colors.textPrimary};
  }

  &[type="number"] {
    -moz-appearance: textfield;
  }

  &[type="number"]::-webkit-outer-spin-button,
  &[type="number"]::-webkit-inner-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }
`;

export default InputBase;
