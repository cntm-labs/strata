import { ref, onUnmounted } from "vue";
import { datasourcesApi } from "@/api/datasources";
import type { Panel } from "@/types";

const rangeMap: Record<string, number> = {
  "5m": 300,
  "15m": 900,
  "30m": 1800,
  "1h": 3600,
  "3h": 10800,
  "6h": 21600,
  "12h": 43200,
  "24h": 86400,
};

export function usePanelData(
  panel: Panel,
  timeRange: string,
  refreshInterval: number,
) {
  const data = ref<unknown>(null);
  const loading = ref(false);
  let timer: ReturnType<typeof setInterval> | null = null;

  async function fetchData() {
    if (!panel.datasource_id) return;
    loading.value = true;
    try {
      const now = Math.floor(Date.now() / 1000);
      const duration = rangeMap[timeRange] || 3600;
      data.value = await datasourcesApi.query(panel.datasource_id, {
        query: panel.query,
        start: (now - duration).toString(),
        end: now.toString(),
        step: Math.max(Math.floor(duration / 250), 15).toString(),
      });
    } finally {
      loading.value = false;
    }
  }

  fetchData();
  if (refreshInterval > 0) {
    timer = setInterval(fetchData, refreshInterval * 1000);
  }

  onUnmounted(() => {
    if (timer) clearInterval(timer);
  });

  return { data, loading, refresh: fetchData };
}
