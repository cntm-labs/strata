<template>
  <div class="card bg-base-200 h-full flex flex-col">
    <div class="card-body p-3 flex flex-col">
      <div class="flex items-center justify-between mb-2">
        <h3 class="text-sm font-semibold truncate">{{ panel.title }}</h3>
        <button
          v-if="editable"
          class="btn btn-ghost btn-xs"
          @click="$emit('edit', panel)"
        >
          <i class="pi pi-pencil" />
        </button>
      </div>
      <div class="flex-1 min-h-0">
        <component :is="panelComponent" :data="data" :config="panel.config" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, type Component } from "vue";
import type { Panel } from "@/types";
import TimeseriesPanel from "./TimeseriesPanel.vue";
import StatPanel from "./StatPanel.vue";
import GaugePanel from "./GaugePanel.vue";
import TablePanel from "./TablePanel.vue";
import BarPanel from "./BarPanel.vue";
import HeatmapPanel from "./HeatmapPanel.vue";
import LogsPanel from "./LogsPanel.vue";
import PiechartPanel from "./PiechartPanel.vue";

const props = defineProps<{
  panel: Panel;
  data: unknown;
  editable?: boolean;
}>();

defineEmits<{ edit: [panel: Panel] }>();

const componentMap: Record<string, Component> = {
  timeseries: TimeseriesPanel,
  stat: StatPanel,
  gauge: GaugePanel,
  table: TablePanel,
  bar: BarPanel,
  heatmap: HeatmapPanel,
  logs: LogsPanel,
  piechart: PiechartPanel,
};

const panelComponent = computed(
  () => componentMap[props.panel.type] || StatPanel,
);
</script>
