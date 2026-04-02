<template>
  <v-chart :option="chartOption" autoresize class="w-full h-full" />
</template>

<script setup lang="ts">
import { computed } from "vue";
import VChart from "vue-echarts";
import { use } from "echarts/core";
import { BarChart } from "echarts/charts";
import { GridComponent, TooltipComponent } from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";

use([BarChart, GridComponent, TooltipComponent, CanvasRenderer]);

const props = defineProps<{ data: unknown; config: Record<string, unknown> }>();

const chartOption = computed(() => {
  const result = (props.data as any)?.data?.result || [];
  const labels = result.map(
    (r: any) => Object.values(r.metric).join(" ") || "unknown",
  );
  const values = result.map((r: any) => parseFloat(r.value?.[1] || "0"));

  return {
    tooltip: { trigger: "axis" },
    xAxis: { type: "category", data: labels },
    yAxis: { type: "value" },
    series: [{ type: "bar", data: values }],
  };
});
</script>
