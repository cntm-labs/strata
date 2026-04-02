<template>
  <div ref="chartEl" class="w-full h-full" />
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from "vue";
import uPlot from "uplot";
import "uplot/dist/uPlot.min.css";

const props = defineProps<{ data: unknown; config: Record<string, unknown> }>();
const chartEl = ref<HTMLElement>();
let chart: uPlot | null = null;

function render() {
  if (!chartEl.value || !props.data) return;
  chart?.destroy();

  const { width, height } = chartEl.value.getBoundingClientRect();
  if (width === 0 || height === 0) return;

  const result = (props.data as any)?.data?.result || [];
  if (result.length === 0) return;

  const timestamps = result[0].values.map((v: [number, string]) => v[0]);
  const series = result.map((r: any) =>
    r.values.map((v: [number, string]) => parseFloat(v[1])),
  );

  const opts: uPlot.Options = {
    width,
    height,
    series: [
      {},
      ...result.map((r: any, i: number) => ({
        label: Object.values(r.metric).join(" ") || `Series ${i + 1}`,
        stroke: `hsl(${i * 60}, 70%, 50%)`,
      })),
    ],
  };

  chart = new uPlot(opts, [timestamps, ...series], chartEl.value);
}

onMounted(render);
watch(() => props.data, render);
onUnmounted(() => chart?.destroy());
</script>
