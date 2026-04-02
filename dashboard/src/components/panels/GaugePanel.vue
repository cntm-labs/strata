<template>
  <v-chart :option="chartOption" autoresize class="w-full h-full" />
</template>

<script setup lang="ts">
import { computed } from "vue";
import VChart from "vue-echarts";
import { use } from "echarts/core";
import { GaugeChart } from "echarts/charts";
import { CanvasRenderer } from "echarts/renderers";

use([GaugeChart, CanvasRenderer]);

const props = defineProps<{ data: unknown; config: Record<string, unknown> }>();

const value = computed(() => {
  const result = (props.data as any)?.data?.result;
  if (!result || result.length === 0) return 0;
  return parseFloat(result[0]?.value?.[1] || "0");
});

const chartOption = computed(() => ({
  series: [
    {
      type: "gauge",
      data: [{ value: value.value }],
      max: (props.config.max as number) || 100,
    },
  ],
}));
</script>
