<template>
  <div>
    <div class="flex items-center justify-between mb-4">
      <div>
        <h1 class="text-2xl font-bold">{{ dashboard?.title }}</h1>
        <p class="text-sm text-base-content/60">{{ dashboard?.description }}</p>
      </div>
      <div class="flex gap-2 items-center">
        <Select
          v-model="timeRange"
          :options="timeRangeOptions"
          optionLabel="label"
          optionValue="value"
        />
        <RouterLink :to="`/dashboards/${slug}/edit`" class="btn btn-ghost">
          <i class="pi pi-pencil mr-1" /> Edit
        </RouterLink>
      </div>
    </div>

    <grid-layout
      v-if="panels.length > 0"
      :layout="gridLayout"
      :col-num="12"
      :row-height="80"
      :is-draggable="false"
      :is-resizable="false"
    >
      <grid-item
        v-for="panel in panels"
        :key="panel.id"
        :x="panel.position.x"
        :y="panel.position.y"
        :w="panel.position.w"
        :h="panel.position.h"
        :i="panel.id"
      >
        <PanelRenderer :panel="panel" :data="panelData[panel.id]" />
      </grid-item>
    </grid-layout>

    <div v-else-if="!loading" class="text-center p-12 text-base-content/60">
      No panels yet.
      <RouterLink :to="`/dashboards/${slug}/edit`" class="link"
        >Add one</RouterLink
      >
    </div>

    <div v-if="loading" class="flex justify-center p-8">
      <ProgressSpinner />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, reactive } from "vue";
import { useRoute } from "vue-router";
import { dashboardsApi } from "@/api/dashboards";
import { datasourcesApi } from "@/api/datasources";
import PanelRenderer from "@/components/panels/PanelRenderer.vue";
import Select from "primevue/select";
import ProgressSpinner from "primevue/progressspinner";
import { GridLayout, GridItem } from "grid-layout-plus";
import type { Dashboard, Panel } from "@/types";

const route = useRoute();
const slug = computed(() => route.params.slug as string);
const dashboard = ref<Dashboard | null>(null);
const panels = ref<Panel[]>([]);
const panelData = reactive<Record<string, unknown>>({});
const loading = ref(true);

const timeRangeOptions = [
  { label: "5m", value: "5m" },
  { label: "15m", value: "15m" },
  { label: "1h", value: "1h" },
  { label: "3h", value: "3h" },
  { label: "6h", value: "6h" },
  { label: "24h", value: "24h" },
];
const timeRange = ref("1h");

const gridLayout = computed(() =>
  panels.value.map((p) => ({
    ...p.position,
    i: p.id,
  })),
);

const rangeMap: Record<string, number> = {
  "5m": 300,
  "15m": 900,
  "1h": 3600,
  "3h": 10800,
  "6h": 21600,
  "24h": 86400,
};

async function fetchPanelData(panel: Panel) {
  if (!panel.datasource_id) return;
  const now = Math.floor(Date.now() / 1000);
  const duration = rangeMap[timeRange.value] || 3600;
  panelData[panel.id] = await datasourcesApi.query(panel.datasource_id, {
    query: panel.query,
    start: (now - duration).toString(),
    end: now.toString(),
    step: Math.max(Math.floor(duration / 250), 15).toString(),
  });
}

onMounted(async () => {
  try {
    const dash = await dashboardsApi.get(slug.value);
    dashboard.value = dash;
    timeRange.value = dash.time_range || "1h";

    const p = await dashboardsApi.listPanels(slug.value);
    panels.value = p;

    await Promise.all(p.map(fetchPanelData));
  } finally {
    loading.value = false;
  }
});
</script>
