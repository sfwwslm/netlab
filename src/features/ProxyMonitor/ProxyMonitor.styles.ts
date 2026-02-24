import styled from "styled-components";
import Input from "@/components/common/Input/Input";

export const ProxyContainer = styled.div`
  display: grid;
  grid-template-columns: minmax(260px, 320px) 1fr;
  height: 100%;
  min-height: 0;
  padding: 1rem;
  gap: 1rem;
  background-color: ${(props) => props.theme.colors.background};
  color: ${(props) => props.theme.colors.textPrimary};
  overflow: hidden;
`;

export const ConfigPanel = styled.div`
  display: flex;
  flex-direction: column;
  gap: 1rem;
  padding: 1rem;
  border: 1px solid ${(props) => props.theme.colors.border};
  border-radius: ${(props) => props.theme.radii.base};
  background-color: ${(props) => props.theme.colors.surface};
  overflow: auto;
`;

export const ConfigSection = styled.div`
  display: flex;
  flex-direction: column;
  gap: 1rem;
`;

export const SectionTitle = styled.h3`
  font-size: 1.05rem;
  font-weight: 600;
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

export const ActionRow = styled.div`
  display: flex;
  gap: 0.5rem;
`;

export const ContentPanel = styled.div`
  display: flex;
  flex-direction: column;
  gap: 1rem;
  min-height: 0;
  overflow: auto;
`;

export const StatsGrid = styled.div`
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
  gap: 0.75rem;
`;

export const StatCard = styled.div`
  padding: 0.85rem;
  border-radius: ${(props) => props.theme.radii.base};
  border: 1px solid ${(props) => props.theme.colors.border};
  background-color: ${(props) => props.theme.colors.surface};
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
`;

export const StatLabel = styled.div`
  font-size: 0.8rem;
  color: ${(props) => props.theme.colors.textSecondary};
`;

export const StatValue = styled.div`
  font-size: 1.1rem;
  font-weight: 600;
`;

export const StatusLine = styled.div`
  font-size: 0.9rem;
  color: ${(props) => props.theme.colors.textSecondary};
  margin-top: 0.5rem;
`;

export const StatusList = styled.div`
  margin-top: 0.5rem;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  font-size: 0.8rem;
  color: ${(props) => props.theme.colors.textSecondary};
  max-height: 160px;
  overflow: auto;
`;

export const StatusItem = styled.div`
  padding-left: 0.25rem;
  border-left: 2px solid ${(props) => props.theme.colors.border};
`;

export const ClientsPanel = styled.div`
  border: 1px solid ${(props) => props.theme.colors.border};
  border-radius: ${(props) => props.theme.radii.base};
  background-color: ${(props) => props.theme.colors.surface};
  display: flex;
  flex-direction: column;
  min-height: 0;
`;

export const ClientsHeader = styled.div`
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  border-bottom: 1px solid ${(props) => props.theme.colors.border};
`;

export const ClientsTitle = styled.div`
  font-size: 1rem;
  font-weight: 600;
`;

export const ClientsBody = styled.div`
  overflow: auto;
  padding: 0.75rem 1rem 1rem;
`;

export const ClientsTable = styled.table`
  width: 100%;
  border-collapse: collapse;
  font-size: 0.85rem;
  border: 1px solid ${(props) => props.theme.colors.border};
`;

export const ClientsHead = styled.thead`
  color: ${(props) => props.theme.colors.textSecondary};
  text-transform: uppercase;
  font-size: 0.7rem;
  letter-spacing: 0.04em;
  border-bottom: 1px solid ${(props) => props.theme.colors.border};

  th {
    padding: 0.5rem 0.4rem;
    text-align: left;
    border-right: 1px solid ${(props) => props.theme.colors.border};

    &:last-child {
      border-right: none;
    }
  }
`;

export const ClientsRow = styled.tr`
  border-bottom: 1px solid ${(props) => props.theme.colors.border};

  &:last-child {
    border-bottom: none;
  }
`;

export const ClientsCell = styled.td`
  padding: 0.5rem 0.4rem;
  text-align: left;
  white-space: nowrap;
  max-width: 240px;
  overflow: hidden;
  text-overflow: ellipsis;
  border-right: 1px solid ${(props) => props.theme.colors.border};

  &:last-child {
    border-right: none;
  }
`;

export const EmptyState = styled.div`
  padding: 1rem 0;
  color: ${(props) => props.theme.colors.textSecondary};
`;
