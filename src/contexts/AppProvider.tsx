import { AppThemeProvider } from "./ThemeContext";
import { ModalProvider } from "./ModalContext";
import { ReactNode } from "react";

export const AppProvider = ({ children }: { children: ReactNode }) => (
  <AppThemeProvider>
    <ModalProvider>{children}</ModalProvider>
  </AppThemeProvider>
);
