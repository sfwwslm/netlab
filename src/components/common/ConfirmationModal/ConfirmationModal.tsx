import React from "react";
import {
  ConfirmationModalContainer,
  ConfirmationTitle,
  ConfirmationMessage,
  ButtonGroup,
  ConfirmButton,
  CancelButton,
} from "./ConfirmationModal.styles";
import { IoAlertCircleOutline } from "react-icons/io5";
import { Overlay } from "@/components/styled/StyledModal";
import { useTranslation } from "react-i18next";

export interface ConfirmationModalProps {
  isOpen: boolean;
  title: string;
  message: string;
  onConfirm: () => void;
  onCancel: () => void;
  confirmText?: string;
  cancelText?: string;
  hideConfirm?: boolean;
}

const ConfirmationModal: React.FC<ConfirmationModalProps> = ({
  isOpen,
  title,
  message,
  onConfirm,
  onCancel,
  confirmText,
  cancelText,
  hideConfirm = false,
}) => {
  if (!isOpen) return null;

  const { t } = useTranslation();
  const finalConfirmText = confirmText || t("button.confirm");
  const finalCancelText = cancelText || t("button.cancel");

  return (
    <Overlay
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{ duration: 0.3 }}
      onClick={onCancel}
    >
      <ConfirmationModalContainer
        initial={{ y: -50, opacity: 0, scale: 0.8 }}
        animate={{ y: 0, opacity: 1, scale: 1 }}
        exit={{ y: -50, opacity: 0, scale: 0.8 }}
        transition={{ type: "spring", damping: 20, stiffness: 300 }}
        onClick={(e) => e.stopPropagation()}
      >
        <ConfirmationTitle>
          <IoAlertCircleOutline
            style={{ verticalAlign: "middle", marginRight: "8px" }}
            size={30}
          />
          {title}
        </ConfirmationTitle>
        <ConfirmationMessage>{message}</ConfirmationMessage>
        <ButtonGroup>
          {!hideConfirm && (
            <ConfirmButton onClick={onConfirm}>
              {finalConfirmText}
            </ConfirmButton>
          )}
          <CancelButton onClick={onCancel}>{finalCancelText}</CancelButton>
        </ButtonGroup>
      </ConfirmationModalContainer>
    </Overlay>
  );
};

export default ConfirmationModal;
