import { api } from "./client";

export interface ExploreHistory {
  id: string;
  datasource_id: string;
  query: string;
  query_type: string;
  created_at: string;
}

export const exploreApi = {
  query: (data: {
    datasource_id: string;
    query: string;
    start?: string;
    end?: string;
    step?: string;
    limit?: number;
  }) => api.post<unknown>("/explore/query", data),

  history: (params?: { datasource_id?: string; limit?: number }) => {
    const qs = new URLSearchParams();
    if (params?.datasource_id) qs.set("datasource_id", params.datasource_id);
    if (params?.limit) qs.set("limit", params.limit.toString());
    const query = qs.toString();
    return api.get<ExploreHistory[]>(
      `/explore/history${query ? `?${query}` : ""}`,
    );
  },

  labels: (datasourceId: string) =>
    api.get<{ data: string[] }>(`/explore/labels/${datasourceId}`),
};
