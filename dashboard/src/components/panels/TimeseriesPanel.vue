<template>
  <div ref="chartEl" class="w-full h-full" />
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue'
import uPlot from 'uplot'
import 'uplot/dist/uPlot.min.css'
import type { PrometheusResponse, PrometheusMetric } from '@/types'

const props = defineProps<{ data: unknown; config: Record<string, unknown> }>()
const chartEl = ref<HTMLElement>()
let chart: uPlot | null = null

function render() {
  if (!chartEl.value || !props.data) return
  chart?.destroy()

  const { width, height } = chartEl.value.getBoundingClientRect()
  if (width === 0 || height === 0) return

  const result = (props.data as PrometheusResponse)?.data?.result ?? []
  const firstMetric = result[0] as PrometheusMetric | undefined
  const firstValues = firstMetric?.values
  if (!firstValues || firstValues.length === 0) return

  const timestamps = firstValues.map((v: [number, string]) => v[0])
  const series = result.map((r: PrometheusMetric) =>
    (r.values || []).map((v: [number, string]) => parseFloat(v[1])),
  )

  const opts: uPlot.Options = {
    width,
    height,
    series: [
      {},
      ...result.map((r: PrometheusMetric, i: number) => ({
        label: Object.values(r.metric).join(' ') || `Series ${i + 1}`,
        stroke: `hsl(${i * 60}, 70%, 50%)`,
      })),
    ],
  }

  chart = new uPlot(opts, [timestamps, ...series], chartEl.value)
}

onMounted(render)
watch(() => props.data, render)
onUnmounted(() => chart?.destroy())
</script>
